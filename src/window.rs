//! Live view window (winit + softbuffer). Runs on the main thread; the session runs on a
//! worker thread and pushes frames through an event-loop proxy. Several lanes (renderers
//! running in parallel) are tiled in a grid.

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use glam::Vec2;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub enum UiEvent {
    Frame { lane: usize, width: u32, height: u32, pixels: Arc<Vec<u8>> },
    Lanes(usize),
    Title(String),
    Exit,
}

/// Keyboard/mouse state shared with the session thread (interactive mode).
#[derive(Default)]
pub struct Input {
    pub held: HashSet<KeyCode>,
    /// Accumulated mouse-look delta in pixels since last read.
    pub look: Vec2,
    pub toggle_mode: bool,
    pub print_camera: bool,
    pub speed_steps: i32,
}

#[derive(Clone)]
pub struct UiHandle {
    proxy: EventLoopProxy<UiEvent>,
    pub input: Arc<Mutex<Input>>,
    closed: Arc<AtomicBool>,
}

impl UiHandle {
    pub fn send(&self, e: UiEvent) {
        let _ = self.proxy.send_event(e);
    }

    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }
}

/// Runs `body` on a new thread with a handle to a window running on this (main) thread.
/// Returns when the window closes; `body`'s result is returned.
pub fn run_with_window<T: Send + 'static>(
    title: &str,
    body: impl FnOnce(UiHandle) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let event_loop = EventLoop::<UiEvent>::with_user_event().build()?;
    let handle = UiHandle {
        proxy: event_loop.create_proxy(),
        input: Default::default(),
        closed: Arc::new(AtomicBool::new(false)),
    };
    let h2 = handle.clone();
    let worker = std::thread::Builder::new().name("session".into()).spawn(move || {
        let r = body(h2.clone());
        h2.send(UiEvent::Exit);
        r
    })?;
    let mut app = App {
        title: title.to_string(),
        window: None,
        surface: None,
        frames: vec![None],
        handle: handle.clone(),
        mouse_down: false,
        last_cursor: None,
    };
    event_loop.run_app(&mut app)?;
    handle.closed.store(true, Ordering::Relaxed);
    worker.join().map_err(|_| anyhow::anyhow!("session thread panicked"))?
}

struct Frame {
    width: u32,
    height: u32,
    pixels: Arc<Vec<u8>>,
}

struct App {
    title: String,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    frames: Vec<Option<Frame>>,
    handle: UiHandle,
    mouse_down: bool,
    last_cursor: Option<(f64, f64)>,
}

impl App {
    fn draw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else { return };
        let size = window.inner_size();
        let (Some(ww), Some(wh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        if surface.resize(ww, wh).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else { return };
        buf.fill(0x00202020);
        let (ww, wh) = (size.width as usize, size.height as usize);
        let n = self.frames.len().max(1);
        let cols = (n as f64).sqrt().ceil() as usize;
        let rows = n.div_ceil(cols);
        let (tw, th) = (ww / cols, wh / rows);
        for (i, f) in self.frames.iter().enumerate() {
            let Some(f) = f else { continue };
            let (fw, fh) = (f.width as usize, f.height as usize);
            if fw == 0 || fh == 0 || tw == 0 || th == 0 {
                continue;
            }
            // Fit preserving aspect ratio, nearest-neighbour.
            let scale = (tw as f64 / fw as f64).min(th as f64 / fh as f64);
            let (dw, dh) = (((fw as f64 * scale) as usize).max(1), ((fh as f64 * scale) as usize).max(1));
            let ox = (i % cols) * tw + (tw - dw) / 2;
            let oy = (i / cols) * th + (th - dh) / 2;
            let xmap: Vec<usize> = (0..dw).map(|x| ((x * fw) / dw).min(fw - 1) * 4).collect();
            for y in 0..dh {
                let sy = ((y * fh) / dh).min(fh - 1);
                let src = &f.pixels[sy * fw * 4..(sy + 1) * fw * 4];
                let dst = &mut buf[(oy + y) * ww + ox..(oy + y) * ww + ox + dw];
                for (d, &sx) in dst.iter_mut().zip(&xmap) {
                    *d = (src[sx] as u32) << 16 | (src[sx + 1] as u32) << 8 | src[sx + 2] as u32;
                }
            }
        }
        let _ = buf.present();
    }
}

impl ApplicationHandler<UiEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(el.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface"));
        self.window = Some(window);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UiEvent) {
        match event {
            UiEvent::Frame { lane, width, height, pixels } => {
                if lane >= self.frames.len() {
                    self.frames.resize_with(lane + 1, || None);
                }
                self.frames[lane] = Some(Frame { width, height, pixels });
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            UiEvent::Lanes(n) => self.frames.resize_with(n.max(1), || None),
            UiEvent::Title(t) => {
                if let Some(w) = &self.window {
                    w.set_title(&t);
                }
            }
            UiEvent::Exit => el.exit(),
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let mut input = self.handle.input.lock().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                self.handle.closed.store(true, Ordering::Relaxed);
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                drop(input);
                self.draw();
            }
            WindowEvent::Resized(_) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key: PhysicalKey::Code(code), state, repeat, .. }, ..
            } => match state {
                ElementState::Pressed => {
                    input.held.insert(code);
                    if !repeat {
                        match code {
                            KeyCode::KeyL => input.toggle_mode = true,
                            KeyCode::KeyP => input.print_camera = true,
                            KeyCode::Escape => {
                                self.handle.closed.store(true, Ordering::Relaxed);
                                el.exit();
                            }
                            _ => {}
                        }
                    }
                }
                ElementState::Released => {
                    input.held.remove(&code);
                }
            },
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.mouse_down = state == ElementState::Pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let (true, Some((lx, ly))) = (self.mouse_down, self.last_cursor) {
                    input.look += Vec2::new((position.x - lx) as f32, (position.y - ly) as f32);
                }
                self.last_cursor = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                input.speed_steps += dy.signum() as i32;
            }
            _ => {}
        }
    }
}
