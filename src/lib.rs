use std::{sync::Arc, time::Instant};

mod state;

use state::State;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::ControlFlow,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

use pollster;

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
