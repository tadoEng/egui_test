//! Shared synthetic GPU viewport, extracted from the reviewed renderer example.
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu;
use egui_wgpu::wgpu;
use wgpu::util::DeviceExt as _;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(18, 25, 35);

const SHADER: &str = r"
struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let light = normalize(vec3<f32>(0.35, -0.55, 0.76));
    let diffuse = 0.28 + 0.72 * abs(dot(normal, light));
    return vec4<f32>(input.color * diffuse, 1.0);
}
";

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

#[derive(Clone, Debug)]
struct MeshData {
    // A geometry revision, not an entity ID. Immutable meshes with different geometry
    // must use different keys; camera changes retain the key and resident buffers.
    key: u64,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    bounds: Bounds,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }
}

impl Bounds {
    fn include(&mut self, point: [f32; 3]) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }

    fn center(self) -> [f32; 3] {
        std::array::from_fn(|axis| self.min[axis].midpoint(self.max[axis]))
    }

    fn radius(self) -> f32 {
        let diagonal = sub(self.max, self.min);
        (dot(diagonal, diagonal).sqrt() * 0.5).max(0.01)
    }
}

#[derive(Default)]
struct MeshBuilder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    bounds: Bounds,
}

impl MeshBuilder {
    fn vertex(&mut self, position: [f32; 3], normal: [f32; 3], color: [f32; 3]) -> u32 {
        self.bounds.include(position);
        let index = u32::try_from(self.vertices.len()).unwrap_or(u32::MAX);
        self.vertices.push(Vertex {
            position,
            normal,
            color,
        });
        index
    }

    fn quad(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3], color: [f32; 3]) {
        let base = self.vertex(corners[0], normal, color);
        for corner in corners.into_iter().skip(1) {
            self.vertex(corner, normal, color);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn box_between(
        &mut self,
        start: [f32; 3],
        end: [f32; 3],
        width: f32,
        depth: f32,
        color: [f32; 3],
    ) {
        let axis = normalized(sub(end, start)).unwrap_or([0.0, 0.0, 1.0]);
        let helper = if axis[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let side = normalized(cross(axis, helper)).unwrap_or([1.0, 0.0, 0.0]);
        let up = normalized(cross(side, axis)).unwrap_or([0.0, 1.0, 0.0]);
        let side = scale(side, width * 0.5);
        let up = scale(up, depth * 0.5);
        let ring = |center: [f32; 3]| {
            [
                add(add(center, side), up),
                add(sub(center, side), up),
                sub(sub(center, side), up),
                add(sub(center, up), side),
            ]
        };
        let a = ring(start);
        let b = ring(end);
        // The ring winds against the member axis; reverse the end cap and each
        // side's previous order so every triangle agrees with its outward normal.
        self.quad([a[0], a[1], a[2], a[3]], scale(axis, -1.0), color);
        self.quad([b[3], b[2], b[1], b[0]], axis, color);
        self.quad(
            [a[0], b[0], b[1], a[1]],
            normalized(up).unwrap_or(up),
            color,
        );
        self.quad(
            [a[2], b[2], b[3], a[3]],
            scale(normalized(up).unwrap_or(up), -1.0),
            color,
        );
        self.quad(
            [a[3], b[3], b[0], a[0]],
            normalized(side).unwrap_or(side),
            color,
        );
        self.quad(
            [a[1], b[1], b[2], a[2]],
            scale(normalized(side).unwrap_or(side), -1.0),
            color,
        );
    }

    fn shell(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3], color: [f32; 3]) {
        self.quad(corners, normal, color);
    }

    fn finish(self, key: u64) -> MeshData {
        MeshData {
            key,
            vertices: self.vertices,
            indices: self.indices,
            bounds: self.bounds,
        }
    }
}

fn synthetic_structure() -> MeshData {
    let mut mesh = MeshBuilder::default();
    let column = [0.22, 0.52, 0.85];
    let beam = [0.35, 0.73, 0.63];
    let shell = [0.72, 0.48, 0.25];
    for x in [-2.6, 2.6] {
        for y in [-1.8, 1.8] {
            mesh.box_between([x, y, 0.0], [x, y, 4.0], 0.48, 0.48, column);
        }
    }
    for y in [-1.8, 1.8] {
        mesh.box_between([-2.6, y, 4.0], [2.6, y, 4.0], 0.36, 0.56, beam);
    }
    for x in [-2.6, 2.6] {
        mesh.box_between([x, -1.8, 4.0], [x, 1.8, 4.0], 0.36, 0.56, beam);
    }
    mesh.shell(
        [
            [-2.6, -1.8, 4.0],
            [2.6, -1.8, 4.0],
            [2.6, 1.8, 4.0],
            [-2.6, 1.8, 4.0],
        ],
        [0.0, 0.0, 1.0],
        shell,
    );
    mesh.finish(1)
}

#[derive(Clone, Copy, Debug)]
struct OrbitCamera {
    target: [f32; 3],
    yaw: f32,
    pitch: f32,
    distance: f32,
    radius: f32,
}

impl OrbitCamera {
    fn fit(bounds: Bounds, aspect: f32) -> Self {
        let radius = bounds.radius();
        let vertical_half = 25.0_f32.to_radians();
        let horizontal_half = (vertical_half.tan() * aspect.max(0.001)).atan();
        let limiting_half = vertical_half.min(horizontal_half);
        Self {
            target: bounds.center(),
            yaw: -0.7,
            pitch: 0.55,
            distance: (radius / limiting_half.sin() * 1.1).max(0.1),
            radius,
        }
    }

    fn orbit(&mut self, delta: egui::Vec2) {
        self.yaw = (self.yaw + delta.x * 0.008).rem_euclid(std::f32::consts::TAU);
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(-limit, limit);
    }

    fn zoom(&mut self, delta: f32) {
        self.distance =
            (self.distance * (-delta * 0.0015).exp()).clamp(self.radius * 0.4, self.radius * 20.0);
    }

    fn basis(self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let back = normalized([-sin_yaw * cos_pitch, -cos_yaw * cos_pitch, sin_pitch])
            .unwrap_or([0.0, -1.0, 0.0]);
        let right = normalized([cos_yaw, -sin_yaw, 0.0]).unwrap_or([1.0, 0.0, 0.0]);
        let up = normalized(cross(back, right)).unwrap_or([0.0, 0.0, 1.0]);
        (right, up, back)
    }

    fn view_projection(self, aspect: f32) -> [[f32; 4]; 4] {
        let (right, up, back) = self.basis();
        let eye = add(self.target, scale(back, self.distance));
        let view = [
            [right[0], up[0], back[0], 0.0],
            [right[1], up[1], back[1], 0.0],
            [right[2], up[2], back[2], 0.0],
            [-dot(right, eye), -dot(up, eye), -dot(back, eye), 1.0],
        ];
        let near = (self.distance - self.radius * 2.0)
            .max(self.distance * 0.001)
            .max(0.001);
        let far = (self.distance + self.radius * 2.0).max(near + 0.01);
        let f = 1.0 / (50.0_f32.to_radians() * 0.5).tan();
        let projection = [
            [f / aspect.max(0.001), 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far / (near - far), -1.0],
            [0.0, 0.0, near * far / (near - far), 0.0],
        ];
        multiply(projection, view)
    }
}

struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
}

struct RendererCore {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh: Option<GpuMesh>,
    uploaded_key: Option<u64>,
}

impl RendererCore {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("native_renderer_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native_renderer_camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("native_renderer_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("native_renderer_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("native_renderer_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x3
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("native_renderer_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            uniform,
            bind_group,
            mesh: None,
            uploaded_key: None,
        }
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &MeshData,
        view_projection: &[[f32; 4]; 4],
    ) {
        if self.uploaded_key != Some(mesh.key) {
            let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("native_renderer_vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("native_renderer_indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.mesh = Some(GpuMesh {
                vertex,
                index,
                index_count: u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX),
            });
            self.uploaded_key = Some(mesh.key);
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(view_projection));
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(mesh) = &self.mesh else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.vertex.slice(..));
        pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}

struct ViewportCallback {
    mesh: Arc<MeshData>,
    view_projection: [[f32; 4]; 4],
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<RendererCore>() {
            renderer.prepare(device, queue, &self.mesh, &self.view_projection);
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(renderer) = resources.get::<RendererCore>() else {
            return;
        };
        let viewport = info.viewport_in_pixels();
        if viewport.width_px == 0 || viewport.height_px == 0 {
            return;
        }
        pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );
        renderer.paint(pass);
    }
}

pub(crate) struct NativeViewport {
    mesh: Arc<MeshData>,
    camera: OrbitCamera,
    fit_pending: bool,
}

impl NativeViewport {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let mesh = Arc::new(synthetic_structure());
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .ok_or_else(|| "native_renderer requires eframe's wgpu renderer".to_owned())?;
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(RendererCore::new(
                &render_state.device,
                render_state.target_format,
            ));
        Ok(Self {
            camera: OrbitCamera::fit(mesh.bounds, 1.0),
            mesh,
            fit_pending: true,
        })
    }
}

impl NativeViewport {
    pub(crate) fn fit(&mut self) {
        self.fit_pending = true;
    }

    pub(crate) fn mesh_counts(&self) -> (usize, usize) {
        (self.mesh.vertices.len(), self.mesh.indices.len() / 3)
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let size = ui.available_size();
        if !size.is_finite() || size.x < 1.0 || size.y < 1.0 {
            return;
        }
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let aspect = rect.width() / rect.height().max(1.0);
        if self.fit_pending {
            self.camera = OrbitCamera::fit(self.mesh.bounds, aspect);
            self.fit_pending = false;
        }
        if response.dragged() {
            self.camera.orbit(ui.input(|input| input.pointer.delta()));
            ctx.request_repaint();
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                self.camera.zoom(scroll);
                ctx.request_repaint();
            }
        }
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                mesh: Arc::clone(&self.mesh),
                view_projection: self.camera.view_projection(aspect),
            },
        ));
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(v: [f32; 3], factor: f32) -> [f32; 3] {
    [v[0] * factor, v[1] * factor, v[2] * factor]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalized(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(v, v).sqrt();
    (length.is_finite() && length > 1e-8).then(|| scale(v, 1.0 / length))
}

fn multiply(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for (column, values) in result.iter_mut().enumerate() {
        for (row, value) in values.iter_mut().enumerate() {
            *value = (0..4).map(|index| a[index][row] * b[column][index]).sum();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_geometry_has_finite_unit_normals_and_valid_indices() {
        let mesh = synthetic_structure();
        assert_eq!(mesh.vertices.len(), 8 * 24 + 4);
        assert_eq!(mesh.indices.len(), 8 * 36 + 6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.position.into_iter().all(f32::is_finite))
        );
        assert!(mesh.vertices.iter().all(|vertex| {
            let length = dot(vertex.normal, vertex.normal).sqrt();
            (length - 1.0).abs() < 1e-5
        }));
        assert!(
            mesh.indices
                .iter()
                .all(|index| (*index as usize) < mesh.vertices.len())
        );
        assert!(mesh.bounds.radius().is_finite() && mesh.bounds.radius() > 0.0);
    }

    fn assert_triangle_winding_matches_normals(mesh: &MeshData) {
        for (triangle, indices) in mesh.indices.chunks_exact(3).enumerate() {
            let vertices: [Vertex; 3] =
                std::array::from_fn(|index| mesh.vertices[indices[index] as usize]);
            let geometric_normal = cross(
                sub(vertices[1].position, vertices[0].position),
                sub(vertices[2].position, vertices[0].position),
            );
            for vertex in vertices {
                assert!(
                    dot(geometric_normal, vertex.normal) > 1e-6,
                    "triangle {triangle} winds against its declared normal: {vertices:?}"
                );
            }
        }
    }

    #[test]
    fn every_fixture_triangle_winds_with_its_declared_normal() {
        assert_triangle_winding_matches_normals(&synthetic_structure());
    }

    fn tilted_shell() -> MeshData {
        let origin = [1.0, -2.0, 0.5];
        let u = [2.0, 1.0, 0.5];
        let v = [-0.5, 1.0, 2.0];
        let normal = normalized(cross(u, v)).expect("nondegenerate tilted plane");
        let mut builder = MeshBuilder::default();
        builder.shell(
            [
                origin,
                add(origin, u),
                add(add(origin, u), v),
                add(origin, v),
            ],
            normal,
            [0.4, 0.6, 0.8],
        );
        builder.finish(3)
    }

    #[test]
    fn tilted_shell_triangles_agree_with_their_non_axis_aligned_normal() {
        let mesh = tilted_shell();
        assert_eq!(mesh.indices.len(), 6);
        assert!(
            mesh.indices
                .iter()
                .all(|index| (*index as usize) < mesh.vertices.len())
        );
        assert!(mesh.vertices.iter().all(|vertex| {
            vertex.position.into_iter().all(f32::is_finite)
                && vertex.normal.into_iter().all(f32::is_finite)
                && (dot(vertex.normal, vertex.normal) - 1.0).abs() < 1e-5
                && vertex
                    .normal
                    .into_iter()
                    .all(|component| component.abs() > 0.1)
        }));
        assert_triangle_winding_matches_normals(&mesh);
    }

    #[test]
    #[should_panic(expected = "winds against its declared normal")]
    fn winding_assertion_rejects_an_inverted_shell_triangle() {
        let mut mesh = tilted_shell();
        mesh.indices.swap(0, 1);
        assert_triangle_winding_matches_normals(&mesh);
    }

    #[test]
    fn rotated_skewed_and_reversed_prisms_have_outward_winding_and_normals() {
        let start = [1.25, -0.75, 2.5];
        for delta in [
            [4.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
            [0.0, 0.0, 4.0],
            [0.0, 0.0, -4.0],
            [-3.0, 2.0, 5.0],
            [2.0, -4.0, -3.0],
            [0.437, 0.0, 0.899],
            [0.435, 0.0, 0.901],
        ] {
            let end = add(start, delta);
            let mut builder = MeshBuilder::default();
            builder.box_between(start, end, 0.36, 0.56, [0.3, 0.5, 0.7]);
            let mesh = builder.finish(2);
            assert_eq!(mesh.indices.len(), 12 * 3);
            assert_triangle_winding_matches_normals(&mesh);
            let center = scale(add(start, end), 0.5);
            for vertex in &mesh.vertices {
                assert!(
                    dot(sub(vertex.position, center), vertex.normal) > 0.0,
                    "prism normal must point away from its center for axis {delta:?}"
                );
            }
        }
    }

    #[test]
    fn camera_remains_finite_at_zoom_pitch_and_resize_limits() {
        let mesh = synthetic_structure();
        for tilt in [-10000.0, 10000.0] {
            for zoom in [-10000.0, 10000.0] {
                let mut camera = OrbitCamera::fit(mesh.bounds, 1.0);
                camera.orbit(egui::vec2(10000.0, tilt));
                camera.zoom(zoom);
                for aspect in [0.001, 0.5, 1.0, 4.0] {
                    assert!(
                        camera
                            .view_projection(aspect)
                            .into_iter()
                            .flatten()
                            .all(f32::is_finite)
                    );
                }
                assert!(camera.pitch.abs() < std::f32::consts::FRAC_PI_2);
            }
        }
    }

    #[test]
    fn fit_contains_every_fixture_vertex_in_tall_and_wide_viewports() {
        let mesh = synthetic_structure();
        for aspect in [0.25, 0.5, 1.0, 2.0, 4.0] {
            let projection = OrbitCamera::fit(mesh.bounds, aspect).view_projection(aspect);
            for vertex in &mesh.vertices {
                let point = [
                    vertex.position[0],
                    vertex.position[1],
                    vertex.position[2],
                    1.0,
                ];
                let clip: [f32; 4] = std::array::from_fn(|row| {
                    (0..4)
                        .map(|column| projection[column][row] * point[column])
                        .sum()
                });
                assert!(clip[3] > 0.0);
                assert!(
                    clip[0].abs() <= clip[3] && clip[1].abs() <= clip[3],
                    "fit clips a vertex at aspect {aspect}: {clip:?}"
                );
                assert!(clip[2] >= 0.0 && clip[2] <= clip[3]);
            }
        }
    }

    #[test]
    #[ignore = "native GPU evidence; run explicitly with --ignored --nocapture"]
    #[expect(
        clippy::print_stderr,
        reason = "Explicit GPU probe records the actual adapter"
    )]
    fn offscreen_gpu_depth_keeps_the_nearer_face() {
        let (device, queue, adapter_name) =
            open_test_device().expect("GPU probe requires an adapter; absence is a failure");
        eprintln!("offscreen GPU adapter: {adapter_name}");
        let size = 64;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let camera = OrbitCamera {
            target: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            distance: 8.0,
            radius: 3.0,
        };
        let (right, up, back) = camera.basis();
        let square = |center: [f32; 3]| {
            [
                add(add(center, scale(right, -2.0)), scale(up, -2.0)),
                add(add(center, scale(right, 2.0)), scale(up, -2.0)),
                add(add(center, scale(right, 2.0)), scale(up, 2.0)),
                add(add(center, scale(right, -2.0)), scale(up, 2.0)),
            ]
        };
        let mut builder = MeshBuilder::default();
        builder.quad(square(scale(back, 1.0)), back, [1.0, 0.0, 0.0]);
        builder.quad(square(scale(back, -1.0)), back, [0.0, 0.0, 1.0]);
        let mesh = builder.finish(99);

        let color = texture(
            &device,
            "native_renderer_test_color",
            format,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            &device,
            "native_renderer_test_depth",
            DEPTH_FORMAT,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut renderer = RendererCore::new(&device, format);
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(1.0));
        let resident_vertex = renderer
            .mesh
            .as_ref()
            .expect("resident mesh")
            .vertex
            .clone();
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(0.5));
        assert_eq!(
            renderer.mesh.as_ref().expect("resident mesh").vertex,
            resident_vertex,
            "orbit/resize must reuse the resident buffer"
        );
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(1.0));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("native_renderer_test_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("native_renderer_test_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer.paint(&mut pass);
        }
        let bytes_per_row = 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native_renderer_test_readback"),
            size: u64::from(bytes_per_row * size),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(size),
                },
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .expect("the GPU should complete the offscreen test");
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the GPU should map the readback buffer")
            .expect("the readback mapping should succeed");
        let data = slice
            .get_mapped_range()
            .expect("the mapped readback range should be available");
        let center = (size / 2 * bytes_per_row + size / 2 * 4) as usize;
        let pixel = &data[center..center + 4];
        assert!(
            pixel[0] > pixel[2] * 3 && pixel[0] > 60,
            "near red face should occlude the later-drawn blue face, got {pixel:?}"
        );
        assert_eq!(&data[..4], &[0, 0, 0, 255], "outside fixture remains clear");
        if let Some(path) = std::env::var_os("NATIVE_GPU_CAPTURE") {
            image::save_buffer(path, &data, size, size, image::ColorType::Rgba8)
                .expect("save synthetic offscreen capture");
        }

        let resident_index = renderer.mesh.as_ref().expect("resident mesh").index.clone();
        let mut replacement = mesh.clone();
        replacement.key += 1;
        replacement.vertices[0].position[0] += 0.25;
        replacement.indices.truncate(3);
        renderer.prepare(&device, &queue, &replacement, &camera.view_projection(1.0));
        let uploaded = renderer.mesh.as_ref().expect("replacement mesh");
        assert_eq!(renderer.uploaded_key, Some(replacement.key));
        assert_ne!(
            uploaded.vertex, resident_vertex,
            "new revision replaces vertices"
        );
        assert_ne!(
            uploaded.index, resident_index,
            "new revision replaces indices"
        );
        assert_eq!(
            uploaded.index_count, 3,
            "new revision replaces the draw count"
        );
    }

    fn open_test_device() -> Option<(wgpu::Device, wgpu::Queue, String)> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let info = adapter.get_info();
        let name = format!(
            "{} ({:?}, {:?}, driver {})",
            info.name, info.device_type, info.backend, info.driver_info
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .ok()?;
        Some((device, queue, name))
    }

    fn texture(
        device: &wgpu::Device,
        label: &str,
        format: wgpu::TextureFormat,
        size: u32,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }
}
