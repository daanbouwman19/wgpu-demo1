use std::{sync::Arc, time::Instant};

use wgpu::PresentMode;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ControlFlow,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

use pollster;

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    mouse_down: bool,
    micros: Instant,
    color_info: ColorInfo,
}

#[derive(Default)]
struct ColorInfo {
    color: wgpu::Color,
    blue_direction: bool,
}

impl<'a> State<'a> {
    async fn new(window: Arc<Window>) -> State<'a> {
        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();

        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window, so this should be safe.
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result in all the colors coming out darker. Ik you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            // present_mode: PresentMode::Fifo,
            present_mode: PresentMode::Mailbox,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self {
            surface,
            device,
            queue,
            config,
            size,
            mouse_down: false,
            micros: Instant::now(),
            color_info: ColorInfo::default(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.color_info.color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn update(&mut self) {
        self.micros = Instant::now();
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let color_info = &mut self.color_info;
                let b = color_info.color.b;
                color_info.color = wgpu::Color {
                    r: position.x as f64 / self.size.width as f64,
                    g: position.y as f64 / self.size.height as f64,
                    b,
                    a: 1.0,
                };
                log::info!("color: {:?}", color_info.color);
                return true;
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                if state.is_pressed() {
                    self.mouse_down = true;
                } else {
                    self.mouse_down = false;
                }
                true;
            }
            _ => {}
        }

        if self.mouse_down {
            self.cycle_blue();
            return true;
        }

        false
    }

    fn cycle_blue(&mut self) {
        let delta_time = self.micros.elapsed().as_nanos() as f64 / 1_000_000_000.0;
        let delta = delta_time * 0.5;
        let color_info = &mut self.color_info;

        if color_info.blue_direction {
            color_info.color.b += delta;
        } else {
            color_info.color.b -= delta;
        }

        if color_info.color.b >= 1.0 {
            color_info.blue_direction = false;
        } else if color_info.color.b <= 0.0 {
            color_info.blue_direction = true;
        }

        log::info!("Color: {:?}", color_info.color);
    }
}

#[derive(Default)]
struct App<'a> {
    window: Option<Arc<Window>>,
    state: Option<State<'a>>,
    frame_count: u64,
    fps_timer: Option<Instant>,
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window_attributes = WindowAttributes::default();
        let window = event_loop.create_window(window_attributes).unwrap();

        let window = Arc::new(window);
        self.window = Some(window.clone());
        let state = pollster::block_on(State::new(window));
        self.state = Some(state);
        self.frame_count = 0;
        self.fps_timer = Some(Instant::now());
        self.window.as_mut().unwrap().request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        let window = self.window.as_ref().unwrap();

        if state.input(&event) {
            window.request_redraw();
        }

        if window_id == window.id() {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key:
                                PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::KeyQ),
                            state: ElementState::Pressed,
                            ..
                        },
                    is_synthetic: false,
                    ..
                } => {
                    event_loop.exit();
                }

                WindowEvent::Resized(physical_size) => {
                    let state = self.state.as_mut().unwrap();
                    state.resize(physical_size);
                }

                WindowEvent::RedrawRequested => {
                    window.request_redraw();
                    state.update();

                    self.frame_count += 1;

                    if self.fps_timer.as_mut().unwrap().elapsed().as_secs() >= 1 {
                        log::info!("FPS: {}", self.frame_count);
                        self.frame_count = 0;
                        self.fps_timer = Some(state.micros.clone());
                    }

                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            state.surface.configure(&state.device, &state.config);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("Out of memory");
                        }
                        Err(wgpu::SurfaceError::Outdated) => {
                            log::error!("Outdated");
                        }
                        Err(e) => {
                            eprintln!("{:?}", e);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn run() {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    _ = event_loop.run_app(&mut app);
}
