//! Shared native wgpu viewport core, moved unchanged in behaviour from
//! `examples/support/native_viewport.rs`: the orbit camera, the WGSL shader,
//! GPU mesh residency keyed by geometry revision, the highlight overlay and the
//! egui paint callback.
//!
//! A host owns its geometry, selection and UI. This crate draws exactly the
//! vertices, indices and highlight indices it is given and never parses a source
//! model, picks, or keeps a selection of its own.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use egui_wgpu::wgpu;
pub use lab_e2k_mesh::Vertex;
use wgpu::util::DeviceExt as _;

pub mod math;
use math::{add, cross, dot, multiply, normalized, scale, sub};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
pub const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(18, 25, 35);
/// Vertical field of view shared by drawing and picking.
pub const FIELD_OF_VIEW_DEGREES: f32 = 50.0;

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

@fragment
fn fs_selected(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.72, 0.12, 1.0);
}
";

/// The geometry revision a resident GPU mesh was uploaded for. Immutable
/// meshes with different geometry must use different keys; camera changes keep
/// the key and the resident buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshKey {
    Lab(u64),
    SourceGeometry([u8; 32]),
}

/// Geometry a host lends the renderer. Uploaded once per [`MeshKey`].
pub trait GpuGeometry: Send + Sync {
    fn key(&self) -> MeshKey;
    fn vertices(&self) -> &[Vertex];
    fn indices(&self) -> &[u32];
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
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
    pub fn include(&mut self, point: [f32; 3]) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }

    pub fn center(self) -> [f32; 3] {
        std::array::from_fn(|axis| self.min[axis].midpoint(self.max[axis]))
    }

    pub fn radius(self) -> f32 {
        let diagonal = sub(self.max, self.min);
        (dot(diagonal, diagonal).sqrt() * 0.5).max(0.01)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    /// Orbit pivot; panning moves it, Fit returns it to `center`.
    pub target: [f32; 3],
    /// Model bounds centre. Clip planes are placed about the model, not the
    /// pivot, so panning never clips (or makes unpickable) drawn geometry.
    pub center: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub radius: f32,
}

impl OrbitCamera {
    pub fn fit(bounds: Bounds, aspect: f32) -> Self {
        let radius = bounds.radius();
        let vertical_half = 25.0_f32.to_radians();
        let horizontal_half = (vertical_half.tan() * aspect.max(0.001)).atan();
        let limiting_half = vertical_half.min(horizontal_half);
        Self {
            target: bounds.center(),
            center: bounds.center(),
            yaw: -0.7,
            pitch: 0.55,
            distance: (radius / limiting_half.sin() * 1.1).max(0.1),
            radius,
        }
    }

    pub fn orbit(&mut self, delta: egui::Vec2) {
        self.yaw = (self.yaw + delta.x * 0.008).rem_euclid(std::f32::consts::TAU);
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(-limit, limit);
    }

    /// Move the pivot in the view's right/up plane so the model follows the
    /// pointer: one screen point is the world size of one point at the pivot
    /// depth. The pivot stays within three model radii of the model centre.
    pub fn pan(&mut self, delta: egui::Vec2, viewport_height: f32) {
        let (right, up, _) = self.basis();
        let world_per_point =
            2.0 * self.distance * (FIELD_OF_VIEW_DEGREES.to_radians() * 0.5).tan()
                / viewport_height.max(1.0);
        let moved = add(
            self.target,
            add(
                scale(right, -delta.x * world_per_point),
                scale(up, delta.y * world_per_point),
            ),
        );
        let offset = sub(moved, self.center);
        let length = dot(offset, offset).sqrt();
        let limit = self.radius * 3.0;
        self.target = if length > limit {
            add(self.center, scale(offset, limit / length))
        } else {
            moved
        };
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance =
            (self.distance * (-delta * 0.0015).exp()).clamp(self.radius * 0.4, self.radius * 20.0);
    }

    pub fn basis(self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let back = normalized([-sin_yaw * cos_pitch, -cos_yaw * cos_pitch, sin_pitch])
            .unwrap_or([0.0, -1.0, 0.0]);
        let right = normalized([cos_yaw, -sin_yaw, 0.0]).unwrap_or([1.0, 0.0, 0.0]);
        let up = normalized(cross(back, right)).unwrap_or([0.0, 0.0, 1.0]);
        (right, up, back)
    }

    pub fn eye(self) -> [f32; 3] {
        add(self.target, scale(self.basis().2, self.distance))
    }

    /// Near/far view depths about the model centre (identical to the old
    /// pivot-based window when unpanned); picking clips with exactly this.
    pub fn clip_planes(self) -> (f32, f32) {
        let back = self.basis().2;
        let depth = dot(sub(self.center, self.eye()), scale(back, -1.0));
        let near = (depth - self.radius * 2.0)
            .max(self.distance * 0.001)
            .max(0.001);
        (near, (depth + self.radius * 2.0).max(near + 0.01))
    }

    /// World ray through normalized device coordinates (x right, y up), with
    /// the distance window that maps to the drawn near/far depth range.
    pub fn ray(self, aspect: f32, ndc: [f32; 2]) -> Option<lab_e2k_mesh::pick::Ray> {
        let (right, up, back) = self.basis();
        let half = (FIELD_OF_VIEW_DEGREES.to_radians() * 0.5).tan();
        let direction = normalized(add(
            add(
                scale(right, ndc[0] * half * aspect.max(0.001)),
                scale(up, ndc[1] * half),
            ),
            scale(back, -1.0),
        ))?;
        let forward = -dot(direction, back);
        let (near, far) = self.clip_planes();
        Some(lab_e2k_mesh::pick::Ray {
            origin: self.eye().map(f64::from),
            direction: direction.map(f64::from),
            min_distance: f64::from(near / forward),
            max_distance: f64::from(far / forward),
        })
    }

    /// Re-centre on new geometry bounds while keeping the view direction and
    /// the relative zoom and pan, so a version switch does not jump the view.
    /// Clip planes follow the new bounds, so no new geometry is clipped.
    pub fn rebase(&mut self, bounds: Bounds) {
        let radius = bounds.radius();
        let ratio = radius / self.radius.max(f32::EPSILON);
        let offset = sub(self.target, self.center);
        self.center = bounds.center();
        self.radius = radius;
        self.target = add(self.center, scale(offset, ratio));
        self.distance = (self.distance * ratio).clamp(radius * 0.4, radius * 20.0);
    }

    pub fn view_projection(self, aspect: f32) -> [[f32; 4]; 4] {
        let (right, up, back) = self.basis();
        let eye = self.eye();
        let view = [
            [right[0], up[0], back[0], 0.0],
            [right[1], up[1], back[1], 0.0],
            [right[2], up[2], back[2], 0.0],
            [-dot(right, eye), -dot(up, eye), -dot(back, eye), 1.0],
        ];
        let (near, far) = self.clip_planes();
        let f = 1.0 / (FIELD_OF_VIEW_DEGREES.to_radians() * 0.5).tan();
        let projection = [
            [f / aspect.max(0.001), 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far / (near - far), -1.0],
            [0.0, 0.0, near * far / (near - far), 0.0],
        ];
        multiply(projection, view)
    }
}

/// The resident vertex and index buffers for one [`MeshKey`].
pub struct GpuMesh {
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
    pub index_count: u32,
}

/// GPU pipelines and the resident mesh. Registered once per wgpu render state
/// (see [`install`]); camera, resize and tab changes never recreate it.
pub struct RendererCore {
    pipeline: wgpu::RenderPipeline,
    selection_pipeline: wgpu::RenderPipeline,
    selection: Option<(Arc<Vec<u32>>, wgpu::Buffer)>,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh: Option<GpuMesh>,
    uploaded_key: Option<MeshKey>,
    stats: Arc<GpuCounters>,
}

#[derive(Default)]
pub struct GpuCounters {
    pub uploads: AtomicU64,
    pub vertex_bytes: AtomicU64,
    pub index_bytes: AtomicU64,
    pub draw_records: AtomicU64,
    pub buffer_creation_cpu_ns: AtomicU64,
    pub selection_uploads: AtomicU64,
    pub selection_bytes: AtomicU64,
    pub selection_draw_records: AtomicU64,
}
#[derive(Clone, Copy, Debug)]
pub struct GpuStats {
    pub uploads: u64,
    pub vertex_bytes: u64,
    pub index_bytes: u64,
    pub draw_records: u64,
    pub buffer_creation_cpu_ns: u64,
    pub selection_uploads: u64,
    pub selection_bytes: u64,
    pub selection_draw_records: u64,
}

impl RendererCore {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
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
        let pipeline =
            Self::create_pipeline(device, target_format, &pipeline_layout, &shader, false);
        let selection_pipeline =
            Self::create_pipeline(device, target_format, &pipeline_layout, &shader, true);
        Self {
            pipeline,
            selection_pipeline,
            selection: None,
            uniform,
            bind_group,
            mesh: None,
            uploaded_key: None,
            stats: Arc::new(GpuCounters::default()),
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        pipeline_layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
        highlighted: bool,
    ) -> wgpu::RenderPipeline {
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x3
        ];
        let descriptor = wgpu::RenderPipelineDescriptor {
            label: Some(if highlighted {
                "native_source_story_highlight"
            } else {
                "native_renderer_pipeline"
            }),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
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
                depth_write_enabled: Some(!highlighted),
                depth_compare: Some(if highlighted {
                    wgpu::CompareFunction::LessEqual
                } else {
                    wgpu::CompareFunction::Less
                }),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(if highlighted {
                    "fs_selected"
                } else {
                    "fs_main"
                }),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        };
        device.create_render_pipeline(&descriptor)
    }

    /// Uploads `mesh` only when its key differs from the resident one; always
    /// writes the camera uniform.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &dyn GpuGeometry,
        view_projection: &[[f32; 4]; 4],
    ) {
        if self.uploaded_key != Some(mesh.key()) {
            {
                self.selection = None;
                self.stats.selection_bytes.store(0, Ordering::Relaxed);
            }
            let started = web_time::Instant::now();
            let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("native_renderer_vertices"),
                contents: bytemuck::cast_slice(mesh.vertices()),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("native_renderer_indices"),
                contents: bytemuck::cast_slice(mesh.indices()),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.mesh = Some(GpuMesh {
                vertex,
                index,
                index_count: u32::try_from(mesh.indices().len()).unwrap_or(u32::MAX),
            });
            self.uploaded_key = Some(mesh.key());
            self.stats.uploads.fetch_add(1, Ordering::Relaxed);
            self.stats.vertex_bytes.store(
                self.mesh.as_ref().map_or(0, |m| m.vertex.size()),
                Ordering::Relaxed,
            );
            self.stats.index_bytes.store(
                self.mesh.as_ref().map_or(0, |m| m.index.size()),
                Ordering::Relaxed,
            );
            self.stats.buffer_creation_cpu_ns.store(
                u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                Ordering::Relaxed,
            );
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(view_projection));
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(mesh) = &self.mesh else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.vertex.slice(..));
        pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        self.stats.draw_records.fetch_add(1, Ordering::Relaxed);
        if let Some((indices, buffer)) = &self.selection {
            pass.set_pipeline(&self.selection_pipeline);
            pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..u32::try_from(indices.len()).unwrap_or(0), 0, 0..1);
            self.stats
                .selection_draw_records
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    /// Highlight overlay: indices into the resident mesh, drawn over it. An
    /// empty list clears it; the same `Arc` is never re-uploaded.
    pub fn prepare_selection(&mut self, device: &wgpu::Device, indices: &Arc<Vec<u32>>) {
        if self
            .selection
            .as_ref()
            .is_some_and(|(old, _)| Arc::ptr_eq(old, indices))
        {
            return;
        }
        if indices.is_empty() {
            self.selection = None;
            self.stats.selection_bytes.store(0, Ordering::Relaxed);
            return;
        }
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("source_story_highlight_indices"),
            contents: bytemuck::cast_slice(indices.as_slice()),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.stats.selection_uploads.fetch_add(1, Ordering::Relaxed);
        self.stats
            .selection_bytes
            .store(buffer.size(), Ordering::Relaxed);
        self.selection = Some((Arc::clone(indices), buffer));
    }

    /// The resident buffers, for evidence tests.
    pub fn resident(&self) -> Option<&GpuMesh> {
        self.mesh.as_ref()
    }
    /// The resident highlight index buffer, for evidence tests.
    pub fn resident_selection(&self) -> Option<&wgpu::Buffer> {
        self.selection.as_ref().map(|(_, buffer)| buffer)
    }
    pub fn uploaded_key(&self) -> Option<MeshKey> {
        self.uploaded_key
    }
    pub fn stats(&self) -> Arc<GpuCounters> {
        Arc::clone(&self.stats)
    }
}

impl GpuCounters {
    pub fn snapshot(&self) -> GpuStats {
        GpuStats {
            uploads: self.uploads.load(Ordering::Relaxed),
            vertex_bytes: self.vertex_bytes.load(Ordering::Relaxed),
            index_bytes: self.index_bytes.load(Ordering::Relaxed),
            draw_records: self.draw_records.load(Ordering::Relaxed),
            buffer_creation_cpu_ns: self.buffer_creation_cpu_ns.load(Ordering::Relaxed),
            selection_uploads: self.selection_uploads.load(Ordering::Relaxed),
            selection_bytes: self.selection_bytes.load(Ordering::Relaxed),
            selection_draw_records: self.selection_draw_records.load(Ordering::Relaxed),
        }
    }
}

/// Registers the renderer with eframe's wgpu state once; returns its counters.
///
/// A second call (another view, a reopened tab) reuses the existing renderer
/// and its resident mesh instead of creating new pipelines.
pub fn install(render_state: &egui_wgpu::RenderState) -> Arc<GpuCounters> {
    let mut renderer = render_state.renderer.write();
    install_into(
        &mut renderer.callback_resources,
        &render_state.device,
        render_state.target_format,
    )
}

/// [`install`] against explicit resources, for hosts and tests that own them.
pub fn install_into(
    resources: &mut egui_wgpu::CallbackResources,
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> Arc<GpuCounters> {
    if let Some(existing) = resources.get::<RendererCore>() {
        return existing.stats();
    }
    let core = RendererCore::new(device, target_format);
    let stats = core.stats();
    resources.insert(core);
    stats
}

/// One frame's draw request: the geometry, its highlight indices and camera.
pub struct ViewportCallback {
    pub mesh: Arc<dyn GpuGeometry>,
    pub selection: Arc<Vec<u32>>,
    pub view_projection: [[f32; 4]; 4],
    pub paints: Arc<AtomicU64>,
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
            renderer.prepare(device, queue, self.mesh.as_ref(), &self.view_projection);
            renderer.prepare_selection(device, &self.selection);
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
        self.paints.fetch_add(1, Ordering::Relaxed);
    }
}

/// Left (or right) drag orbits; middle drag, or Shift + left drag for
/// trackpads, pans; the wheel zooms while hovered. Returns whether the camera
/// moved, so the host can request a repaint.
pub fn navigate(
    camera: &mut OrbitCamera,
    ui: &egui::Ui,
    response: &egui::Response,
    viewport_height: f32,
) -> bool {
    let delta = ui.input(|input| input.pointer.delta());
    let shift = ui.input(|input| input.modifiers.shift);
    let mut moved = false;
    if response.dragged_by(egui::PointerButton::Middle)
        || (shift && response.dragged_by(egui::PointerButton::Primary))
    {
        camera.pan(delta, viewport_height);
        moved = true;
    } else if response.dragged_by(egui::PointerButton::Primary)
        || response.dragged_by(egui::PointerButton::Secondary)
    {
        camera.orbit(delta);
        moved = true;
    }
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll.abs() > f32::EPSILON {
            camera.zoom(scroll);
            moved = true;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebase_keeps_direction_and_relative_view_and_tracks_new_bounds() {
        let first = Bounds {
            min: [-1.0, -1.0, 0.0],
            max: [1.0, 1.0, 4.0],
        };
        let mut camera = OrbitCamera::fit(first, 1.5);
        camera.orbit(egui::vec2(40.0, -15.0));
        camera.pan(egui::vec2(25.0, 10.0), 600.0);
        let (yaw, pitch) = (camera.yaw, camera.pitch);
        let relative = camera.distance / camera.radius;
        let second = Bounds {
            min: [10.0, 10.0, 0.0],
            max: [14.0, 14.0, 8.0],
        };
        camera.rebase(second);
        assert_eq!(
            (camera.yaw, camera.pitch),
            (yaw, pitch),
            "view direction kept"
        );
        assert_eq!(
            camera.center,
            second.center(),
            "clip planes follow the new model"
        );
        assert!(
            (camera.radius - second.radius()).abs() < 1e-5,
            "radius updated"
        );
        assert!(
            ((camera.distance / camera.radius) - relative).abs() < 1e-4,
            "relative zoom kept"
        );
        let (near, far) = camera.clip_planes();
        assert!(near > 0.0 && far > near, "valid clip window after rebase");
    }

    struct Lab {
        key: MeshKey,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    }
    impl GpuGeometry for Lab {
        fn key(&self) -> MeshKey {
            self.key
        }
        fn vertices(&self) -> &[Vertex] {
            &self.vertices
        }
        fn indices(&self) -> &[u32] {
            &self.indices
        }
    }
    fn triangle(key: u64) -> Lab {
        let vertex = |position| Vertex {
            position,
            normal: [0.0, 0.0, 1.0],
            color: [0.5; 3],
        };
        Lab {
            key: MeshKey::Lab(key),
            vertices: vec![
                vertex([0.0, 0.0, 0.0]),
                vertex([1.0, 0.0, 0.0]),
                vertex([0.0, 1.0, 0.0]),
            ],
            indices: vec![0, 1, 2],
        }
    }

    /// Residency contract a host relies on (TALOS tab and version switches):
    /// one renderer per resources, uploads only on a new key, highlight
    /// uploads only for a new index list.
    #[test]
    #[ignore = "needs a GPU adapter; run with --include-ignored (Mesa lavapipe works)"]
    fn install_once_upload_per_geometry_key_and_highlight_per_list() {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("GPU test requires an adapter; absence is a failure");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .expect("device");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut resources = egui_wgpu::CallbackResources::default();
        let first = install_into(&mut resources, &device, format);
        let second = install_into(&mut resources, &device, format);
        assert!(
            Arc::ptr_eq(&first, &second),
            "a second install reuses the renderer"
        );

        let camera = OrbitCamera::fit(
            Bounds {
                min: [0.0; 3],
                max: [1.0, 1.0, 0.0],
            },
            1.0,
        );
        let renderer = resources.get_mut::<RendererCore>().expect("installed");
        let (a, b) = (triangle(1), triangle(2));
        let mut prepare = |mesh: &Lab| {
            renderer.prepare(&device, &queue, mesh, &camera.view_projection(1.0));
        };
        for (mesh, uploads) in [(&a, 1), (&a, 1), (&b, 2), (&b, 2), (&a, 3)] {
            prepare(mesh);
            assert_eq!(first.snapshot().uploads, uploads, "after {:?}", mesh.key);
        }
        let renderer = resources.get_mut::<RendererCore>().expect("installed");
        let highlight = Arc::new(vec![0, 1, 2]);
        renderer.prepare_selection(&device, &highlight);
        renderer.prepare_selection(&device, &highlight);
        assert_eq!(
            first.snapshot().selection_uploads,
            1,
            "same list, no re-upload"
        );
        renderer.prepare_selection(&device, &Arc::new(Vec::new()));
        assert_eq!(first.snapshot().selection_bytes, 0, "empty list clears");
    }
}
