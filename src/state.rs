use crate::{camera, gui, texture};
use cgmath::prelude::*;
use egui_wgpu::wgpu::util::DeviceExt;
use egui_winit::winit::{event::*, keyboard::PhysicalKey, window::Window};
use tracing::{debug, debug_span, error, trace};

const NUM_INSTANCES_PER_ROW: u32 = 15;
const INSTANCE_DISPLACEMENT: cgmath::Vector3<f32> = cgmath::Vector3::new(
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> egui_wgpu::wgpu::VertexBufferLayout<'static> {
        use std::mem;
        egui_wgpu::wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as egui_wgpu::wgpu::BufferAddress,
            step_mode: egui_wgpu::wgpu::VertexStepMode::Vertex,
            attributes: &[
                egui_wgpu::wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: egui_wgpu::wgpu::VertexFormat::Float32x3,
                },
                egui_wgpu::wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as egui_wgpu::wgpu::BufferAddress,
                    shader_location: 1,
                    format: egui_wgpu::wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.5, 0.5, 0.0],
        tex_coords: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.0],
        tex_coords: [1.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        tex_coords: [0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        tex_coords: [1.0, 1.0],
    },
];

#[rustfmt::skip]
const INDICES: &[u16] = &[
    1, 0, 2,
    1, 2, 3
];

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        use cgmath::SquareMatrix;

        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &camera::Camera, projection: &camera::Projection) {
        self.view_proj = (projection.calc_matrix() * camera.calc_matrix()).into();
    }
}

struct Instance {
    position: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
}

impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation))
            .into(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
}

impl InstanceRaw {
    fn desc() -> egui_wgpu::wgpu::VertexBufferLayout<'static> {
        use std::mem;
        egui_wgpu::wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as egui_wgpu::wgpu::BufferAddress,
            step_mode: egui_wgpu::wgpu::VertexStepMode::Instance,
            attributes: &[
                egui_wgpu::wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: egui_wgpu::wgpu::VertexFormat::Float32x4,
                },
                egui_wgpu::wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as egui_wgpu::wgpu::BufferAddress,
                    shader_location: 6,
                    format: egui_wgpu::wgpu::VertexFormat::Float32x4,
                },
                egui_wgpu::wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as egui_wgpu::wgpu::BufferAddress,
                    shader_location: 7,
                    format: egui_wgpu::wgpu::VertexFormat::Float32x4,
                },
                egui_wgpu::wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as egui_wgpu::wgpu::BufferAddress,
                    shader_location: 8,
                    format: egui_wgpu::wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct Status {
    pub fps: f32,
    pub fps_avg: f32,
    pub delta: u128,
    pub cap_frame_rate: bool,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            fps: 0.0,
            fps_avg: 0.0,
            delta: 0,
            cap_frame_rate: true,
        }
    }
}

pub struct State<'a> {
    pub size: egui_winit::winit::dpi::PhysicalSize<u32>,
    pub egui: gui::EguiRenderer,
    pub window: &'a Window,
    pub status: Status,
    pub mouse_pressed: bool,
    clear_color: egui_wgpu::wgpu::Color,
    surface: egui_wgpu::wgpu::Surface<'a>,
    device: egui_wgpu::wgpu::Device,
    queue: egui_wgpu::wgpu::Queue,
    config: egui_wgpu::wgpu::SurfaceConfiguration,
    render_pipeline: egui_wgpu::wgpu::RenderPipeline,
    vertex_buffer: egui_wgpu::wgpu::Buffer,
    index_buffer: egui_wgpu::wgpu::Buffer,
    num_indices: u32,
    diffuse_bind_group: egui_wgpu::wgpu::BindGroup,
    _diffuse_texture: texture::Texture,
    camera: camera::Camera,
    projection: camera::Projection,
    pub camera_controller: camera::CameraController,
    camera_uniform: CameraUniform,
    camera_buffer: egui_wgpu::wgpu::Buffer,
    camera_bind_group: egui_wgpu::wgpu::BindGroup,
    instances: Vec<Instance>,
    instance_buffer: egui_wgpu::wgpu::Buffer,
    depth_texture: texture::Texture,
}

impl<'a> State<'a> {
    pub async fn new(window: &'a Window) -> State<'a> {
        let span = debug_span!("State::new");
        let _enter = span.enter();

        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            error!("Window has a width or height of 0");
            panic!();
        } else {
            trace!("Window size: {:?}", size);
        }

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = egui_wgpu::wgpu::Instance::new(egui_wgpu::wgpu::InstanceDescriptor {
            backends: egui_wgpu::wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        trace!("Instance created");

        let surface = match instance.create_surface(window) {
            Ok(surface) => surface,
            Err(e) => {
                error!("Failed to create surface: {:?}", e);
                panic!();
            }
        };
        trace!("Surface created");

        let adapter = match instance
            .request_adapter(&egui_wgpu::wgpu::RequestAdapterOptions {
                power_preference: egui_wgpu::wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
        {
            Some(adapter) => adapter,
            None => {
                error!("Failed to find an adapter");
                panic!();
            }
        };
        debug!("Adapter created: {:?}", adapter.get_info());

        let (device, queue) = match adapter
            .request_device(
                &egui_wgpu::wgpu::DeviceDescriptor {
                    required_features: egui_wgpu::wgpu::Features::empty(),
                    required_limits: egui_wgpu::wgpu::Limits::default(),
                    label: None,
                    // memory_hints: Default::default(),
                },
                None,
            )
            .await
        {
            Ok((device, queue)) => (device, queue),
            Err(e) => {
                error!("Failed to create device and queue: {:?}", e);
                panic!();
            }
        };
        trace!("Device and queue created");

        let surface_caps = surface.get_capabilities(&adapter);
        // sRGB is a color space that is standard for the web and most displays
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = egui_wgpu::wgpu::SurfaceConfiguration {
            usage: egui_wgpu::wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        trace!("Surface configuration created: {:?}", config);

        surface.configure(&device, &config);
        let diffuse_bytes = include_bytes!("happy-tree.png");
        let diffuse_texture =
            texture::Texture::from_bytes(&device, &queue, diffuse_bytes, "happy-tree.png").unwrap();
        trace!("Diffuse texture created");

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let texture_bind_group_layout =
            device.create_bind_group_layout(&egui_wgpu::wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    egui_wgpu::wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: egui_wgpu::wgpu::ShaderStages::FRAGMENT,
                        ty: egui_wgpu::wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: egui_wgpu::wgpu::TextureViewDimension::D2,
                            sample_type: egui_wgpu::wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                        },
                        count: None,
                    },
                    egui_wgpu::wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: egui_wgpu::wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: egui_wgpu::wgpu::BindingType::Sampler(
                            egui_wgpu::wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });
        let diffuse_bind_group = device.create_bind_group(&egui_wgpu::wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                egui_wgpu::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: egui_wgpu::wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                egui_wgpu::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: egui_wgpu::wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });
        debug!("Diffuse bind group created");

        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).flat_map(move |x| {
                    (0..NUM_INSTANCES_PER_ROW)
                        .map(move |y| {
                            let position = cgmath::Vector3 {
                                x: x as f32,
                                y: y as f32,
                                z: z as f32,
                            } - INSTANCE_DISPLACEMENT;

                            let rotation = if position.is_zero() {
                                cgmath::Quaternion::from_axis_angle(
                                    cgmath::Vector3::unit_z(),
                                    cgmath::Deg(0.0),
                                )
                            } else {
                                cgmath::Quaternion::from_axis_angle(
                                    position.normalize(),
                                    cgmath::Deg(30.0),
                                )
                            };

                            Instance { position, rotation }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances
            .iter()
            .map(Instance::to_raw)
            .collect::<Vec<InstanceRaw>>();
        let instance_buffer =
            device.create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
            });

        let camera = camera::Camera::new((0.0, 5.0, 20.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection =
            camera::Projection::new(config.width, config.height, cgmath::Deg(45.0), 0.1, 100.0);
        let camera_controller = camera::CameraController::new(10.0, 1.0);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);
        let camera_buffer =
            device.create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: egui_wgpu::wgpu::BufferUsages::UNIFORM
                    | egui_wgpu::wgpu::BufferUsages::COPY_DST,
            });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&egui_wgpu::wgpu::BindGroupLayoutDescriptor {
                entries: &[egui_wgpu::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: egui_wgpu::wgpu::ShaderStages::VERTEX,
                    ty: egui_wgpu::wgpu::BindingType::Buffer {
                        ty: egui_wgpu::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });
        let camera_bind_group = device.create_bind_group(&egui_wgpu::wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[egui_wgpu::wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });
        trace!("Camera created");

        trace!("Creating render pipeline");
        let shader = device.create_shader_module(egui_wgpu::wgpu::include_wgsl!("shader.wgsl"));
        debug!("Shader created");
        let render_pipeline_layout =
            device.create_pipeline_layout(&egui_wgpu::wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline =
            device.create_render_pipeline(&egui_wgpu::wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: egui_wgpu::wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc(), InstanceRaw::desc()],
                    // compilation_options: egui_wgpu::wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(egui_wgpu::wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(egui_wgpu::wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(egui_wgpu::wgpu::BlendState::REPLACE),
                        write_mask: egui_wgpu::wgpu::ColorWrites::ALL,
                    })],
                    // compilation_options: egui_wgpu::wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: egui_wgpu::wgpu::PrimitiveState {
                    topology: egui_wgpu::wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: egui_wgpu::wgpu::FrontFace::Ccw,
                    cull_mode: Some(egui_wgpu::wgpu::Face::Back),
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: egui_wgpu::wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(egui_wgpu::wgpu::DepthStencilState {
                    format: texture::Texture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: egui_wgpu::wgpu::CompareFunction::Less,
                    stencil: egui_wgpu::wgpu::StencilState::default(),
                    bias: egui_wgpu::wgpu::DepthBiasState::default(),
                }),
                multisample: egui_wgpu::wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                // cache: None,
            });
        trace!("Render pipeline created");

        let vertex_buffer =
            device.create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
            });
        trace!("Vertex buffer created");
        let index_buffer =
            device.create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: egui_wgpu::wgpu::BufferUsages::INDEX,
            });
        let num_indices = INDICES.len() as u32;
        trace!("Index buffer created");

        let egui = gui::EguiRenderer::new(&device, window);
        trace!("Egui renderer created");

        debug!("State created successfully");
        Self {
            size,
            clear_color: egui_wgpu::wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            surface,
            device,
            queue,
            config,
            window,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            diffuse_bind_group,
            _diffuse_texture: diffuse_texture,
            camera,
            projection,
            camera_controller,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            instances,
            instance_buffer,
            depth_texture,
            egui,
            status: Status::default(),
            mouse_pressed: false,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: egui_winit::winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
            self.projection.resize(new_size.width, new_size.height);
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        ..
                    },
                ..
            } => self.camera_controller.process_keyboard(*key, *state),
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            _ => false,
        }
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    pub fn render(&mut self) -> Result<(), egui_wgpu::wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.device
                .create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let mut render_pass =
                encoder.begin_render_pass(&egui_wgpu::wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(egui_wgpu::wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: egui_wgpu::wgpu::Operations {
                            load: egui_wgpu::wgpu::LoadOp::Clear(self.clear_color),
                            store: egui_wgpu::wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(
                        egui_wgpu::wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_texture.view,
                            depth_ops: Some(egui_wgpu::wgpu::Operations {
                                load: egui_wgpu::wgpu::LoadOp::Clear(1.0),
                                store: egui_wgpu::wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        },
                    ),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(
                self.index_buffer.slice(..),
                egui_wgpu::wgpu::IndexFormat::Uint16,
            );
            render_pass.draw_indexed(0..self.num_indices, 0, 0..self.instances.len() as _);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: 1.0,
        };

        self.egui.render(
            &self.device,
            &self.queue,
            &mut encoder,
            self.window,
            &view,
            &screen_descriptor,
            |ui| {
                egui::Window::new("Debug").show(&ui, |ui| {
                    ui.label(format!("FPS: {:.2}", self.status.fps));
                    ui.label(format!("Avg FPS: {:.2}", self.status.fps_avg));
                    ui.label(format!(
                        "Delta Time: {} µs ({} ms)",
                        self.status.delta,
                        self.status.delta / 1000
                    ));
                    ui.separator();
                    ui.label("Window");
                    ui.label(format!("Width: {}", self.size.width));
                    ui.label(format!("Height: {}", self.size.height));
                    ui.separator();
                    ui.label("Camera");
                    ui.label(format!("Camera Position: {:?}", self.camera.position));
                    ui.label(format!("Camera Yaw: {:?}", self.camera.yaw));
                    ui.label(format!("Camera Pitch: {:?}", self.camera.pitch));
                    ui.separator();
                    ui.label("Projection");
                    ui.label(format!("Aspect: {}", self.projection.aspect));
                    ui.label(format!("Fovy: {:?}", self.projection.fovy));
                    ui.label(format!("Znear: {}", self.projection.znear));
                    ui.label(format!("Zfar: {}", self.projection.zfar));
                    ui.separator();
                    ui.label("Instances");
                    ui.label(format!("Instances per row: {}", NUM_INSTANCES_PER_ROW));
                    ui.label(format!("Amount of Instances: {}", self.instances.len()));
                    ui.label(format!("Amount triangles: {}", self.instances.len() * 2));
                });
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
