//! Shared GPU viewport, extracted from the reviewed renderer example.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[cfg(not(feature = "real-geometry"))]
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu;
use egui_wgpu::wgpu;
#[cfg(feature = "real-geometry")]
use lab_e2k_mesh::Vertex;
use wgpu::util::DeviceExt as _;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(18, 25, 35);
/// Vertical field of view shared by drawing and picking.
const FIELD_OF_VIEW_DEGREES: f32 = 50.0;

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[cfg(not(feature = "real-geometry"))]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

#[cfg(feature = "real-geometry")]
#[expect(
    clippy::cast_possible_truncation,
    reason = "Local bounds were derived from these already validated f32 vertices; conversion round-trips"
)]
fn source_mesh(source: lab_e2k_mesh::Mesh) -> MeshData {
    let source = source.into_render_parts();
    #[cfg(feature = "real-workbench")]
    let pick =
        lab_e2k_mesh::pick::PickIndex::new(&source.vertices, &source.indices, &source.entities);
    MeshData {
        key: MeshKey::SourceGeometry(source.revision.geometry_sha256),
        vertices: source.vertices,
        indices: source.indices,
        #[cfg(feature = "real-workbench")]
        source_entities: source.entities,
        #[cfg(feature = "real-workbench")]
        revision: Some(source.revision),
        #[cfg(feature = "real-workbench")]
        pick,
        bounds: Bounds {
            min: source.bounds.min.map(|v| v as f32),
            max: source.bounds.max.map(|v| v as f32),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MeshKey {
    Lab(u64),
    #[cfg(feature = "real-geometry")]
    SourceGeometry([u8; 32]),
}

#[derive(Clone, Debug)]
struct MeshData {
    // A geometry revision, not an entity ID. Immutable meshes with different geometry
    // must use different keys; camera changes retain the key and resident buffers.
    key: MeshKey,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    #[cfg(feature = "real-workbench")]
    source_entities: Vec<lab_e2k_mesh::EntityMesh>,
    /// Source revision for selection scoping; `None` for authored lab meshes.
    #[cfg(feature = "real-workbench")]
    revision: Option<lab_e2k_mesh::Revision>,
    #[cfg(feature = "real-workbench")]
    pick: lab_e2k_mesh::pick::PickIndex,
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
            key: MeshKey::Lab(key),
            vertices: self.vertices,
            indices: self.indices,
            #[cfg(feature = "real-workbench")]
            source_entities: Vec::new(),
            #[cfg(feature = "real-workbench")]
            revision: None,
            #[cfg(feature = "real-workbench")]
            pick: lab_e2k_mesh::pick::PickIndex::default(),
            bounds: self.bounds,
        }
    }
}

#[cfg(feature = "real-workbench")]
fn story_indices(mesh: &MeshData, story: Option<&str>) -> Result<Vec<u32>, &'static str> {
    let Some(story) = story else {
        return Ok(Vec::new());
    };
    let mut indices = Vec::new();
    for entity in mesh
        .source_entities
        .iter()
        .filter(|entity| entity.story == story)
    {
        let range = entity.indices.start as usize..entity.indices.end as usize;
        let source = mesh
            .indices
            .get(range)
            .ok_or("Invalid source index range")?;
        if source.len() % 3 != 0 || indices.len().saturating_add(source.len()) > 16 * 1024 * 1024 {
            return Err("Story highlight exceeds its 64 MiB index budget");
        }
        indices.extend_from_slice(source);
    }
    Ok(indices)
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

    fn eye(self) -> [f32; 3] {
        add(self.target, scale(self.basis().2, self.distance))
    }

    /// Near/far view depths; picking must clip with exactly the drawn window.
    fn clip_planes(self) -> (f32, f32) {
        let near = (self.distance - self.radius * 2.0)
            .max(self.distance * 0.001)
            .max(0.001);
        (near, (self.distance + self.radius * 2.0).max(near + 0.01))
    }

    /// World ray through normalized device coordinates (x right, y up), with
    /// the distance window that maps to the drawn near/far depth range.
    #[cfg(feature = "real-workbench")]
    fn ray(self, aspect: f32, ndc: [f32; 2]) -> Option<lab_e2k_mesh::pick::Ray> {
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

    fn view_projection(self, aspect: f32) -> [[f32; 4]; 4] {
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

struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
}

struct RendererCore {
    pipeline: wgpu::RenderPipeline,
    #[cfg(feature = "real-workbench")]
    selection_pipeline: wgpu::RenderPipeline,
    #[cfg(feature = "real-workbench")]
    selection: Option<(Arc<Vec<u32>>, wgpu::Buffer)>,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh: Option<GpuMesh>,
    uploaded_key: Option<MeshKey>,
    stats: Arc<GpuCounters>,
}

#[derive(Default)]
struct GpuCounters {
    uploads: AtomicU64,
    vertex_bytes: AtomicU64,
    index_bytes: AtomicU64,
    draw_records: AtomicU64,
    buffer_creation_cpu_ns: AtomicU64,
    #[cfg(feature = "real-workbench")]
    selection_uploads: AtomicU64,
    #[cfg(feature = "real-workbench")]
    selection_bytes: AtomicU64,
    #[cfg(feature = "real-workbench")]
    selection_draw_records: AtomicU64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct GpuStats {
    pub(crate) uploads: u64,
    pub(crate) vertex_bytes: u64,
    pub(crate) index_bytes: u64,
    pub(crate) draw_records: u64,
    pub(crate) buffer_creation_cpu_ns: u64,
    #[cfg(feature = "real-workbench")]
    pub(crate) selection_uploads: u64,
    #[cfg(feature = "real-workbench")]
    pub(crate) selection_bytes: u64,
    #[cfg(feature = "real-workbench")]
    pub(crate) selection_draw_records: u64,
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
        let pipeline =
            Self::create_pipeline(device, target_format, &pipeline_layout, &shader, false);
        #[cfg(feature = "real-workbench")]
        let selection_pipeline =
            Self::create_pipeline(device, target_format, &pipeline_layout, &shader, true);
        Self {
            pipeline,
            #[cfg(feature = "real-workbench")]
            selection_pipeline,
            #[cfg(feature = "real-workbench")]
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

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &MeshData,
        view_projection: &[[f32; 4]; 4],
    ) {
        if self.uploaded_key != Some(mesh.key) {
            #[cfg(feature = "real-workbench")]
            {
                self.selection = None;
                self.stats.selection_bytes.store(0, Ordering::Relaxed);
            }
            let started = web_time::Instant::now();
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

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(mesh) = &self.mesh else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.vertex.slice(..));
        pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        self.stats.draw_records.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "real-workbench")]
        if let Some((indices, buffer)) = &self.selection {
            pass.set_pipeline(&self.selection_pipeline);
            pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..u32::try_from(indices.len()).unwrap_or(0), 0, 0..1);
            self.stats
                .selection_draw_records
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    #[cfg(feature = "real-workbench")]
    fn prepare_selection(&mut self, device: &wgpu::Device, indices: &Arc<Vec<u32>>) {
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
}

struct ViewportCallback {
    mesh: Arc<MeshData>,
    #[cfg(feature = "real-workbench")]
    selection: Arc<Vec<u32>>,
    view_projection: [[f32; 4]; 4],
    paints: Arc<AtomicU64>,
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
            #[cfg(feature = "real-workbench")]
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

pub(crate) struct NativeViewport {
    mesh: Arc<MeshData>,
    camera: OrbitCamera,
    fit_pending: bool,
    paints: Arc<AtomicU64>,
    stats: Arc<GpuCounters>,
    #[cfg(feature = "real-workbench")]
    selected_story: Option<String>,
    #[cfg(feature = "real-workbench")]
    selection_error: Option<&'static str>,
    #[cfg(feature = "real-workbench")]
    selection: Arc<Vec<u32>>,
    #[cfg(feature = "real-workbench")]
    story_indices: Arc<Vec<u32>>,
    #[cfg(feature = "real-workbench")]
    picking: Picking,
}

/// Individual element hover/selection, scoped to the mesh revision and the
/// host's load generation. The story highlight is separate and never counts as
/// an element selection.
#[cfg(feature = "real-workbench")]
#[derive(Default)]
struct Picking {
    generation: u64,
    selection: lab_e2k_mesh::pick::Selection,
    hovered: Option<lab_e2k_mesh::pick::Hit>,
    clicked: Option<lab_e2k_mesh::pick::Hit>,
    last_pick_cpu_us: Option<u128>,
}

impl NativeViewport {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        #[cfg(feature = "real-geometry")]
        {
            Self::new_with_source(cc, None)
        }
        #[cfg(not(feature = "real-geometry"))]
        {
            Self::with_mesh(cc, Arc::new(synthetic_structure()))
        }
    }

    #[cfg(feature = "real-geometry")]
    pub(crate) fn new_with_source(
        cc: &eframe::CreationContext<'_>,
        source: Option<lab_e2k_mesh::Mesh>,
    ) -> Result<Self, String> {
        let mesh = source.map_or_else(synthetic_structure, source_mesh);
        Self::with_mesh(cc, Arc::new(mesh))
    }

    fn with_mesh(cc: &eframe::CreationContext<'_>, mesh: Arc<MeshData>) -> Result<Self, String> {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .ok_or_else(|| "native_renderer requires eframe's wgpu renderer".to_owned())?;
        let renderer = RendererCore::new(&render_state.device, render_state.target_format);
        let stats = Arc::clone(&renderer.stats);
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(renderer);
        Ok(Self {
            camera: OrbitCamera::fit(mesh.bounds, 1.0),
            mesh,
            fit_pending: true,
            paints: Arc::new(AtomicU64::new(0)),
            stats,
            #[cfg(feature = "real-workbench")]
            selected_story: None,
            #[cfg(feature = "real-workbench")]
            selection_error: None,
            #[cfg(feature = "real-workbench")]
            selection: Arc::new(Vec::new()),
            #[cfg(feature = "real-workbench")]
            story_indices: Arc::new(Vec::new()),
            #[cfg(feature = "real-workbench")]
            picking: Picking::default(),
        })
    }
}

impl NativeViewport {
    pub(crate) fn gpu_stats(&self) -> GpuStats {
        GpuStats {
            uploads: self.stats.uploads.load(Ordering::Relaxed),
            vertex_bytes: self.stats.vertex_bytes.load(Ordering::Relaxed),
            index_bytes: self.stats.index_bytes.load(Ordering::Relaxed),
            draw_records: self.stats.draw_records.load(Ordering::Relaxed),
            buffer_creation_cpu_ns: self.stats.buffer_creation_cpu_ns.load(Ordering::Relaxed),
            #[cfg(feature = "real-workbench")]
            selection_uploads: self.stats.selection_uploads.load(Ordering::Relaxed),
            #[cfg(feature = "real-workbench")]
            selection_bytes: self.stats.selection_bytes.load(Ordering::Relaxed),
            #[cfg(feature = "real-workbench")]
            selection_draw_records: self.stats.selection_draw_records.load(Ordering::Relaxed),
        }
    }
    /// Callback records, not presented/completed frames or GPU draw-call timing.
    pub(crate) fn callback_paints(&self) -> u64 {
        self.paints.load(Ordering::Relaxed)
    }
    pub(crate) fn fit(&mut self) {
        self.fit_pending = true;
    }

    pub(crate) fn mesh_counts(&self) -> (usize, usize) {
        (self.mesh.vertices.len(), self.mesh.indices.len() / 3)
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui, selected_story: Option<&str>) {
        #[cfg(feature = "real-workbench")]
        if self.selected_story.as_deref() != selected_story {
            self.selected_story = selected_story.map(str::to_owned);
            match story_indices(&self.mesh, selected_story) {
                Ok(indices) => {
                    self.story_indices = Arc::new(indices);
                    self.selection_error = None;
                }
                Err(error) => {
                    self.story_indices = Arc::new(Vec::new());
                    self.selection_error = Some(error);
                }
            }
            self.refresh_overlay();
        }
        #[cfg(feature = "real-workbench")]
        if let Some(error) = self.selection_error {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        #[cfg(not(feature = "real-workbench"))]
        let _unused_selection = selected_story;
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
        #[cfg(feature = "real-workbench")]
        self.interact(ui, &response, rect, aspect);
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                mesh: Arc::clone(&self.mesh),
                #[cfg(feature = "real-workbench")]
                selection: Arc::clone(&self.selection),
                view_projection: self.camera.view_projection(aspect),
                paints: Arc::clone(&self.paints),
            },
        ));
        let stats = self.gpu_stats();
        #[cfg(feature = "real-workbench")]
        let selection_label = format!(
            " · selected indices {} bytes / {} uploads / {} draw records",
            stats.selection_bytes, stats.selection_uploads, stats.selection_draw_records
        );
        #[cfg(not(feature = "real-workbench"))]
        let selection_label = "";
        ui.painter().with_clip_rect(rect).text(
            rect.left_bottom() + egui::vec2(10.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "{} draw records · {} buffer uploads · {} mesh bytes · CPU buffer creation {} µs{}",
                stats.draw_records,
                stats.uploads,
                stats.vertex_bytes + stats.index_bytes,
                stats.buffer_creation_cpu_ns / 1000,
                selection_label
            ),
            egui::FontId::monospace(10.0),
            egui::Color32::LIGHT_GRAY,
        );
        #[cfg(feature = "real-workbench")]
        self.inspector(ui, rect);
    }
}

#[cfg(feature = "real-workbench")]
impl NativeViewport {
    /// The host's load generation; a change expires the element selection even
    /// when the new load has byte-identical geometry.
    pub(crate) fn set_load_generation(&mut self, generation: u64) {
        if self.picking.generation != generation {
            self.picking.generation = generation;
            self.picking.hovered = None;
            self.picking.clicked = None;
            self.refresh_overlay();
        }
    }

    fn scope(&self) -> Option<lab_e2k_mesh::pick::Scope> {
        self.mesh.revision.map(|r| (r, self.picking.generation))
    }

    pub(crate) fn selected_entities(&self) -> Vec<&lab_e2k_mesh::EntityMesh> {
        self.scope().map_or_else(Vec::new, |scope| {
            self.picking
                .selection
                .entities(scope)
                .iter()
                .filter_map(|&i| self.mesh.source_entities.get(i))
                .collect()
        })
    }

    fn interact(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        aspect: f32,
    ) {
        let Some(scope) = self.scope() else {
            return;
        };
        let at = |pos: egui::Pos2| {
            let ndc = [
                (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
            ];
            pick_at(&self.mesh, self.camera, aspect, ndc)
        };
        let moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        match response.hover_pos() {
            Some(pos) if moved || self.picking.hovered.is_none() => {
                self.picking.hovered = at(pos);
            }
            None => self.picking.hovered = None,
            Some(_) => {}
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let started = web_time::Instant::now();
            let hit = at(pos);
            let toggle = ui.input(|i| i.modifiers.command);
            self.picking
                .selection
                .click(scope, hit.map(|h| h.entity), toggle);
            self.picking.clicked = hit;
            self.refresh_overlay();
            self.picking.last_pick_cpu_us = Some(started.elapsed().as_micros());
            ui.ctx().request_repaint();
        }
    }

    /// Element selection takes precedence over the story highlight. Only the
    /// selected index ranges are uploaded; vertex buffers are never re-sent.
    fn refresh_overlay(&mut self) {
        let mut indices = Vec::new();
        for entity in self.selected_entities() {
            if let Some(range) = self
                .mesh
                .indices
                .get(entity.indices.start as usize..entity.indices.end as usize)
            {
                indices.extend_from_slice(range);
            }
        }
        self.selection = if indices.is_empty() {
            Arc::clone(&self.story_indices)
        } else {
            Arc::new(indices)
        };
    }

    fn inspector(&self, ui: &egui::Ui, rect: egui::Rect) {
        let selected = self.selected_entities();
        let mut lines = vec![format!(
            "Selected {} element(s){}",
            selected.len(),
            self.picking
                .last_pick_cpu_us
                .map_or_else(String::new, |us| format!(" · last pick CPU {us} µs"))
        )];
        let coincident = self.picking.clicked.map_or(0, |h| h.coincident);
        lines.extend(selected.iter().take(6).map(|e| describe(e)));
        if selected.len() > 6 {
            lines.push(format!("… {} more", selected.len() - 6));
        }
        if coincident > 0 {
            lines.push(format!(
                "⚠ {coincident} other element(s) at the same depth; pick is ambiguous"
            ));
        }
        if let Some(entity) = self
            .picking
            .hovered
            .and_then(|h| self.mesh.source_entities.get(h.entity))
        {
            lines.push(format!("Hover: {}", describe(entity)));
        }
        ui.painter().with_clip_rect(rect).text(
            rect.left_top() + egui::vec2(10.0, 8.0),
            egui::Align2::LEFT_TOP,
            lines.join("\n"),
            egui::FontId::monospace(11.0),
            egui::Color32::WHITE,
        );
    }
}

/// Nearest visible element under normalized device coordinates.
#[cfg(feature = "real-workbench")]
fn pick_at(
    mesh: &MeshData,
    camera: OrbitCamera,
    aspect: f32,
    ndc: [f32; 2],
) -> Option<lab_e2k_mesh::pick::Hit> {
    let ray = camera.ray(aspect, ndc)?;
    mesh.pick.pick(
        &mesh.vertices,
        &mesh.indices,
        &mesh.source_entities,
        &ray,
        |_| true,
    )
}

/// Source attributes and fidelity for one entity; never inferred engineering data.
#[cfg(feature = "real-workbench")]
fn describe(entity: &lab_e2k_mesh::EntityMesh) -> String {
    let metres = |v: Option<f64>| v.map_or_else(|| "—".to_owned(), |v| format!("{v:.3} m"));
    let detail = if let Some(frame) = &entity.frame {
        format!(
            "section {} · {} · material {} · D {} × B {} · {:?} {:?}",
            frame.section.as_deref().unwrap_or("unassigned"),
            frame.shape.as_deref().unwrap_or("shape unknown"),
            frame.material.as_deref().unwrap_or("—"),
            metres(frame.depth_m),
            metres(frame.width_m),
            frame.fidelity,
            frame.reasons,
        )
    } else if let Some(shell) = &entity.shell {
        format!(
            "section {} · material {} · t {} (not drawn) · insertion {}",
            shell.section.as_deref().unwrap_or("unassigned"),
            shell.material.as_deref().unwrap_or("—"),
            metres(shell.thickness_m),
            shell.insertion_point.as_deref().unwrap_or("undeclared"),
        )
    } else {
        String::new()
    };
    format!(
        "{} {:?} · {:?} · {detail}",
        entity.key.as_stable_str(),
        entity.class,
        entity.mode
    )
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
        replacement.key = MeshKey::Lab(100);
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

    #[cfg(feature = "real-geometry")]
    #[test]
    fn source_conversion_moves_buffers_and_preserves_the_complete_geometry_key() {
        let source = lab_e2k_mesh::build(
            lab_e2k_mesh::public_fixture::E2K.as_bytes(),
            Default::default(),
        )
        .expect("public source");
        let vertices = source.vertices().as_ptr();
        let indices = source.indices().as_ptr();
        #[cfg(feature = "real-workbench")]
        let entities = source.entities().as_ptr();
        let key = source.revision().geometry_sha256;
        let mesh = source_mesh(source);
        assert_eq!(
            mesh.vertices.as_ptr(),
            vertices,
            "transfer must not clone vertex storage"
        );
        assert_eq!(
            mesh.indices.as_ptr(),
            indices,
            "transfer must not clone index storage"
        );
        assert_eq!(mesh.key, MeshKey::SourceGeometry(key));
        #[cfg(feature = "real-workbench")]
        assert_eq!(
            mesh.source_entities.as_ptr(),
            entities,
            "source identity/ranges move with buffers"
        );
        assert_eq!(std::mem::size_of::<Vertex>(), 36);
        assert_triangle_winding_matches_normals(&mesh);
    }

    #[cfg(feature = "real-workbench")]
    fn project(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
        let clip: [f32; 4] =
            std::array::from_fn(|row| (0..3).map(|c| m[c][row] * p[c]).sum::<f32>() + m[3][row]);
        [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]]
    }

    #[cfg(feature = "real-workbench")]
    #[test]
    fn pick_rays_pass_through_drawn_pixels_inside_the_drawn_depth_window() {
        let mesh = synthetic_structure();
        let mut checked = 0;
        for (aspect, yaw, pitch, zoom) in [
            (1.0, 0.0, 0.0, 0.0),
            (1.8, 1.1, 0.7, 300.0),
            (0.45, -2.4, -0.9, -500.0),
        ] {
            let mut camera = OrbitCamera::fit(mesh.bounds, aspect);
            camera.orbit(egui::vec2(yaw / 0.008, pitch / 0.008));
            camera.zoom(zoom);
            let m = camera.view_projection(aspect);
            for p in mesh.vertices.iter().step_by(97).map(|v| v.position) {
                let ndc = project(m, p);
                if ndc.iter().any(|v| v.abs() > 1.0) || ndc[2] < 0.0 {
                    continue;
                }
                let ray = camera.ray(aspect, [ndc[0], ndc[1]]).expect("ray");
                let to = std::array::from_fn::<f64, 3, _>(|i| f64::from(p[i]) - ray.origin[i]);
                let t: f64 = (0..3).map(|i| to[i] * ray.direction[i]).sum();
                let miss: f64 = (0..3)
                    .map(|i| (to[i] - t * ray.direction[i]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(
                    miss < 1.0e-3 * t.max(1.0),
                    "ray misses drawn point by {miss}"
                );
                assert!(t >= ray.min_distance - 1e-3 && t <= ray.max_distance + 1e-3);
                checked += 1;
            }
        }
        assert!(checked > 30, "only {checked} drawn points were checked");
    }

    #[cfg(feature = "real-workbench")]
    #[test]
    fn clicking_a_projected_source_surface_picks_that_entity_and_empty_space_picks_none() {
        let source = lab_e2k_mesh::build(
            lab_e2k_mesh::public_fixture::E2K.as_bytes(),
            Default::default(),
        )
        .expect("public source");
        let mesh = source_mesh(source);
        let aspect = 1.5;
        let camera = OrbitCamera::fit(mesh.bounds, aspect);
        let floor = mesh
            .source_entities
            .iter()
            .position(|e| e.class == lab_e2k_mesh::SourceClass::FloorShell)
            .expect("floor");
        let range = &mesh.indices[mesh.source_entities[floor].indices.start as usize
            ..mesh.source_entities[floor].indices.end as usize];
        // Centroid of the floor's first triangle, which no frame covers from this view.
        let p: [f32; 3] = std::array::from_fn(|axis| {
            range[..3]
                .iter()
                .map(|&i| mesh.vertices[i as usize].position[axis])
                .sum::<f32>()
                / 3.0
        });
        let ndc = project(camera.view_projection(aspect), p);
        let hit = pick_at(&mesh, camera, aspect, [ndc[0], ndc[1]]).expect("floor hit");
        assert_eq!(hit.entity, floor);
        assert_eq!(
            pick_at(&mesh, camera, aspect, [0.99, 0.99]),
            None,
            "empty corner"
        );
        let described = describe(&mesh.source_entities[floor]);
        assert!(described.contains("FloorShell") && described.contains("not drawn"));
    }

    #[cfg(feature = "real-workbench")]
    #[test]
    fn source_story_highlight_uses_exact_attributed_ranges_and_refuses_invalid_ranges() {
        let source = lab_e2k_mesh::build(
            lab_e2k_mesh::public_fixture::E2K.as_bytes(),
            Default::default(),
        )
        .expect("public source");
        let mut mesh = source_mesh(source);
        assert_eq!(
            story_indices(&mesh, Some("Roof")).expect("all Roof entities"),
            mesh.indices
        );
        assert!(
            story_indices(&mesh, Some("Base"))
                .expect("story without assigned entities")
                .is_empty()
        );
        assert!(
            story_indices(&mesh, Some("roof"))
                .expect("no inferred case-folding")
                .is_empty()
        );
        mesh.source_entities[1].story = "Base".into();
        let assigned = mesh.source_entities[1].indices.clone();
        assert_eq!(
            story_indices(&mesh, Some("Base")).expect("exact range"),
            mesh.indices[assigned.start as usize..assigned.end as usize]
        );
        assert_eq!(
            story_indices(&mesh, Some("Roof"))
                .expect("remaining Roof ranges")
                .len(),
            mesh.indices.len() - (assigned.end - assigned.start) as usize
        );
        mesh.source_entities[1].indices.end = u32::MAX;
        assert!(
            story_indices(&mesh, Some("Base")).is_err(),
            "never slice an invalid range or allocate from it"
        );
    }

    #[cfg(feature = "real-workbench")]
    #[test]
    #[ignore = "public GPU highlight/readback evidence; run explicitly with --ignored --nocapture"]
    fn public_source_story_highlight_reuses_geometry_and_clears_overlay() {
        let (device, queue, _adapter) = open_test_device().expect("GPU adapter required");
        let mesh = source_mesh(
            lab_e2k_mesh::build(
                lab_e2k_mesh::public_fixture::E2K.as_bytes(),
                Default::default(),
            )
            .expect("public source"),
        );
        let camera = OrbitCamera::fit(mesh.bounds, 1.0);
        let mut renderer = RendererCore::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(1.0));
        let resident = renderer.mesh.as_ref().expect("resident").vertex.clone();
        let selected = Arc::new(story_indices(&mesh, Some("Roof")).expect("source indices"));
        renderer.prepare_selection(&device, &selected);
        let selected_buffer = renderer
            .selection
            .as_ref()
            .expect("highlight buffer")
            .1
            .clone();
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(1.0));
        renderer.prepare_selection(&device, &selected);
        assert_eq!(renderer.mesh.as_ref().expect("resident").vertex, resident);
        assert_eq!(
            renderer.selection.as_ref().expect("highlight").1,
            selected_buffer
        );
        assert_eq!(renderer.stats.uploads.load(Ordering::Relaxed), 1);
        assert_eq!(renderer.stats.selection_uploads.load(Ordering::Relaxed), 1);
        let highlighted = read_public_rgba(&device, &queue, &renderer);
        let gold = |bytes: &[u8]| {
            bytes
                .chunks_exact(4)
                .filter(|p| p[0] > 240 && p[1] > 160 && p[1] < 210 && p[2] < 60)
                .count()
        };
        assert!(
            gold(&highlighted) > 100,
            "gold source-story overlay must reach mapped GPU pixels"
        );
        if let Some(path) = std::env::var_os("E2K_WORKBENCH_HIGHLIGHT_CAPTURE") {
            image::save_buffer(path, &highlighted, 256, 256, image::ColorType::Rgba8)
                .expect("public capture");
        }
        renderer.prepare_selection(&device, &Arc::new(Vec::new()));
        assert!(renderer.selection.is_none());
        assert_eq!(renderer.stats.selection_bytes.load(Ordering::Relaxed), 0);
        let cleared = read_public_rgba(&device, &queue, &renderer);
        assert!(
            gold(&cleared) < gold(&highlighted),
            "clear restores unhighlighted geometry"
        );
        assert_eq!(
            renderer.stats.uploads.load(Ordering::Relaxed),
            1,
            "selection never reuploads vertices"
        );
    }

    #[cfg(feature = "real-workbench")]
    fn read_public_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &RendererCore,
    ) -> Vec<u8> {
        let size = 256;
        let color = texture(
            device,
            "public_highlight_color",
            wgpu::TextureFormat::Rgba8Unorm,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            device,
            "public_highlight_depth",
            DEPTH_FORMAT,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("public_highlight_pass"),
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
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("public_highlight_readback"),
            size: u64::from(size * size * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
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
        let (send, recv) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _sent = send.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .expect("GPU completion");
        recv.recv_timeout(std::time::Duration::from_secs(10))
            .expect("map callback")
            .expect("map success");
        let mapped = slice.get_mapped_range().expect("mapped pixels");
        let pixels = mapped.to_vec();
        drop(mapped);
        readback.unmap();
        pixels
    }

    #[cfg(feature = "real-geometry")]
    #[test]
    #[ignore = "native GPU public E2K evidence; run explicitly with --ignored --nocapture"]
    #[expect(
        clippy::print_stderr,
        reason = "Explicit GPU probe records the actual adapter"
    )]
    fn public_e2k_upload_readback_and_geometry_cache() {
        let (device, queue, adapter) = open_test_device().expect("GPU adapter required");
        eprintln!("public E2K GPU adapter: {adapter}");
        let source = lab_e2k_mesh::build(
            lab_e2k_mesh::public_fixture::E2K.as_bytes(),
            Default::default(),
        )
        .expect("public source");
        let source_key = source.revision().source_sha256;
        let expected_bytes = source.buffer_bytes() as u64;
        let mesh = source_mesh(source);
        let camera = OrbitCamera::fit(mesh.bounds, 1.0);
        let mut renderer = RendererCore::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(1.0));
        let resident = renderer.mesh.as_ref().expect("resident").vertex.clone();
        let resident_index = renderer.mesh.as_ref().expect("resident").index.clone();
        let comment_only = format!(
            "{}\n$ public metadata comment\n",
            lab_e2k_mesh::public_fixture::E2K
        );
        let same = lab_e2k_mesh::build(comment_only.as_bytes(), Default::default())
            .expect("comment-only source");
        assert_ne!(source_key, same.revision().source_sha256);
        let same_mesh = source_mesh(same);
        assert_eq!(mesh.key, same_mesh.key);
        renderer.prepare(&device, &queue, &same_mesh, &camera.view_projection(0.5));
        assert_eq!(renderer.mesh.as_ref().expect("resident").vertex, resident);
        assert_eq!(
            renderer.mesh.as_ref().expect("resident").index,
            resident_index
        );
        assert_eq!(renderer.stats.uploads.load(Ordering::Relaxed), 1);
        assert_eq!(resident.size() + resident_index.size(), expected_bytes);
        renderer.prepare(&device, &queue, &mesh, &camera.view_projection(1.0));

        let size = 256;
        let color = texture(
            &device,
            "public_e2k_color",
            wgpu::TextureFormat::Rgba8Unorm,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            &device,
            "public_e2k_depth",
            DEPTH_FORMAT,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("public_e2k_pass"),
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
        let bytes_per_row = size * 4;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("public_e2k_readback"),
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
        let (send, recv) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .expect("GPU completion");
        recv.recv_timeout(std::time::Duration::from_secs(10))
            .expect("map callback")
            .expect("map success");
        let data = slice.get_mapped_range().expect("mapped range");
        let geometry_pixels = data
            .chunks_exact(4)
            .filter(|p| p[..3].iter().any(|v| *v > 20))
            .count();
        assert!(
            geometry_pixels > 100,
            "source geometry must reach actual GPU readback"
        );
        assert_eq!(&data[..4], &[0, 0, 0, 255]);
        assert_eq!(renderer.stats.draw_records.load(Ordering::Relaxed), 1);
        if let Some(path) = std::env::var_os("E2K_PUBLIC_GPU_CAPTURE") {
            image::save_buffer(path, &data, size, size, image::ColorType::Rgba8)
                .expect("save public fixture capture");
        }
        drop(data);
        readback.unmap();

        let mut changed = mesh.clone();
        let MeshKey::SourceGeometry(mut digest) = changed.key else {
            panic!("source key expected")
        };
        digest[31] ^= 1;
        changed.key = MeshKey::SourceGeometry(digest);
        renderer.prepare(&device, &queue, &changed, &camera.view_projection(1.0));
        assert_eq!(
            renderer.stats.uploads.load(Ordering::Relaxed),
            2,
            "full digest including last byte invalidates buffers"
        );
        assert_ne!(
            renderer.mesh.as_ref().expect("replacement").vertex,
            resident
        );
        assert_ne!(
            renderer.mesh.as_ref().expect("replacement").index,
            resident_index
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
