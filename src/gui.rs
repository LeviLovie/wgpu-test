use egui::Context;
use egui_wgpu::Renderer;
use egui_winit::State;

pub struct EguiRenderer {
    context: Context,
    state: State,
    renderer: Renderer,
}

impl EguiRenderer {
    pub fn new(
        device: &egui_wgpu::wgpu::Device,
        window: &egui_winit::winit::window::Window,
    ) -> Self {
        let egui_context = Context::default();
        let id = egui_context.viewport_id();

        egui_context.set_visuals(egui::Visuals::dark());

        let egui_state = State::new(egui_context.clone(), id, window, None, None);

        let egui_renderer = Renderer::new(
            device,
            egui_wgpu::wgpu::TextureFormat::Rgba8UnormSrgb,
            None,
            1,
        );

        Self {
            context: egui_context,
            state: egui_state,
            renderer: egui_renderer,
        }
    }

    pub fn handle_input(
        &mut self,
        window: &egui_winit::winit::window::Window,
        event: &egui_winit::winit::event::WindowEvent,
    ) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn render(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        window: &egui_winit::winit::window::Window,
        window_surface_view: &egui_wgpu::wgpu::TextureView,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        mut run_ui: impl FnMut(&Context),
    ) {
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.context.run(raw_input, |ui| run_ui(ui));

        self.state
            .handle_platform_output(&window, full_output.platform_output);

        let tris = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);

        let mut rpass = encoder.begin_render_pass(&egui_wgpu::wgpu::RenderPassDescriptor {
            label: Some("egui"),
            color_attachments: &[Some(egui_wgpu::wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Load,
                    store: egui_wgpu::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.renderer.render(&mut rpass, &tris, screen_descriptor);
        drop(rpass);

        for x in full_output.textures_delta.free {
            self.renderer.free_texture(&x);
        }
    }
}
