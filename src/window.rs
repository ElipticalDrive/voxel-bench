//! Live view window (winit + a wgpu surface). Runs on the main thread; the session runs on a
//! worker thread and pushes frames through an event-loop proxy. Several lanes (renderers
//! running in parallel) are tiled in a grid. Float frames (an `hdr` post) are presented in
//! HDR where the display offers an HDR surface (H toggles it, `[` / `]` set their brightness).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use glam::Vec2;
use mr_core::lighting::PixelFormat;
use mr_gpu::wgpu;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub enum UiEvent {
    /// A lane's frame in its worker's format (RGBA16F extended sRGB for an `hdr` post).
    Frame { lane: usize, width: u32, height: u32, format: PixelFormat, pixels: Arc<Vec<u8>> },
    Lanes(usize),
    Title(String),
    /// The frames' 1.0 on an HDR surface, relative to SDR white.
    Gain(f32),
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
    /// The gain `[` / `]` set, since last read.
    pub gain: Option<f32>,
    /// How the window presents ([`Gpu::status`]), since last read.
    pub status: Option<String>,
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
        gpu: None,
        frames: vec![None],
        handle: handle.clone(),
        mouse_down: false,
        last_cursor: None,
        gain: 1.0,
    };
    event_loop.run_app(&mut app)?;
    handle.closed.store(true, Ordering::Relaxed);
    worker.join().map_err(|_| anyhow::anyhow!("session thread panicked"))?
}

struct Frame {
    width: u32,
    height: u32,
    format: PixelFormat,
    pixels: Arc<Vec<u8>>,
}

/// How the window's surface takes the frames' (extended) sRGB values.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Encoding {
    /// SDR into a UNORM format: the values clamped, as they are.
    Sdr = 0,
    /// SDR into an `*Srgb` format (the hardware encodes): the values clamped and decoded.
    SdrSrgbFormat = 1,
    /// scRGB: linear light, 1.0 = SDR white (80 nits on Windows).
    ExtendedLinear = 2,
    /// Extended sRGB: the sRGB curve continued past 1, 1.0 as scRGB's.
    Extended = 3,
    /// HDR10: BT.2020 primaries, PQ (absolute nits).
    Pq = 4,
}

impl Encoding {
    fn name(self) -> &'static str {
        match self {
            Self::Sdr | Self::SdrSrgbFormat => "SDR",
            Self::ExtendedLinear => "scRGB",
            Self::Extended => "extended sRGB",
            Self::Pq => "HDR10 (PQ)",
        }
    }

    fn hdr(self) -> bool {
        matches!(self, Self::ExtendedLinear | Self::Extended | Self::Pq)
    }

    /// The shader's scale of linear light: the frames' 1.0 in the colour space's units, at
    /// `gain` times SDR white. `sdr_white` is the display's SDR white in nits where the
    /// platform reports it (Windows, whose scRGB 1.0 is 80 nits); elsewhere the extended
    /// spaces' 1.0 is SDR white and PQ assumes 203 nits (BT.2408).
    fn scale(self, gain: f32, sdr_white: Option<f32>) -> f32 {
        match self {
            Self::Sdr | Self::SdrSrgbFormat => 1.0,
            Self::ExtendedLinear | Self::Extended => gain * sdr_white.map_or(1.0, |n| n / 80.0),
            Self::Pq => gain * sdr_white.unwrap_or(203.0) / 10000.0,
        }
    }
}

/// The surface configuration: HDR if the display offers it (`hdr`), else SDR. HDR order:
/// extended sRGB (the frames' own encoding), scRGB, HDR10.
fn choose(caps: &wgpu::SurfaceCapabilities, hdr: bool) -> Option<(wgpu::TextureFormat, wgpu::SurfaceColorSpace, Encoding)> {
    use wgpu::{SurfaceColorSpace as C, TextureFormat as F};
    if hdr {
        for (f, c, e) in [
            (F::Rgba16Float, C::ExtendedSrgb, Encoding::Extended),
            (F::Rgba16Float, C::ExtendedSrgbLinear, Encoding::ExtendedLinear),
            (F::Rgb10a2Unorm, C::Bt2100Pq, Encoding::Pq),
            (F::Rgba16Float, C::Bt2100Pq, Encoding::Pq),
        ] {
            if c.to_color_spaces().is_some_and(|s| caps.color_spaces(f).contains(s)) {
                return Some((f, c, e));
            }
        }
    }
    let f = caps.formats.iter().find(|f| !f.is_srgb()).or(caps.formats.first())?;
    Some((*f, wgpu::SurfaceColorSpace::Auto, if f.is_srgb() { Encoding::SdrSrgbFormat } else { Encoding::Sdr }))
}

const PRESENT_WGSL: &str = r"
struct Present { encoding: u32, scale: f32, gamma22: u32 }
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var<uniform> p: Present;

struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }

@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return V(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
}

/// The frames' values to linear light, by the curve the compositor decodes SDR windows
/// with, so HDR frames' SDR range looks as in SDR.
fn decode(c: vec3<f32>) -> vec3<f32> {
    let a = abs(c);
    if p.gamma22 != 0u {
        return sign(c) * pow(a, vec3(2.2));
    }
    return sign(c) * select(pow((a + 0.055) / 1.055, vec3(2.4)), a / 12.92, a <= vec3(0.04045));
}

fn encode(l: vec3<f32>) -> vec3<f32> {
    let a = abs(l);
    return sign(l) * select(1.055 * pow(a, vec3(1.0 / 2.4)) - 0.055, a * 12.92, a <= vec3(0.0031308));
}

fn pq(l: vec3<f32>) -> vec3<f32> {
    let y = pow(clamp(l, vec3(0.0), vec3(1.0)), vec3(0.1593017578125));
    return pow((0.8359375 + 18.8515625 * y) / (1.0 + 18.6875 * y), vec3(78.84375));
}

/// The presented value of an (extended) sRGB-encoded frame value.
fn present(c: vec3<f32>) -> vec3<f32> {
    switch p.encoding {
        case 0u: { return clamp(c, vec3(0.0), vec3(1.0)); }
        case 1u: { return decode(clamp(c, vec3(0.0), vec3(1.0))); }
        case 2u: { return decode(c) * p.scale; }
        case 3u: { return encode(decode(c) * p.scale); }
        default: {
            // BT.709 to BT.2020 primaries.
            let m = mat3x3<f32>(0.6274, 0.0691, 0.0164, 0.3293, 0.9195, 0.0880, 0.0433, 0.0114, 0.8956);
            return pq(m * max(decode(c), vec3(0.0)) * p.scale);
        }
    }
}

@fragment fn fs(v: V) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(frame));
    let c = textureLoad(frame, min(vec2<i32>(v.uv * vec2<f32>(size)), size - 1), 0).rgb;
    return vec4<f32>(present(c), 1.0);
}
";

/// The window's GPU side: a surface (HDR where offered), one texture per lane in the
/// frame's format, one draw per tile.
struct Gpu {
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    encoding: Encoding,
    /// HDR requested (H toggles).
    hdr: bool,
    /// The frames' 1.0 on an HDR surface, relative to SDR white (`[` / `]`).
    gain: f32,
    /// The display's SDR white in nits, where the platform reports it.
    sdr_white: Option<f32>,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    uniform: wgpu::Buffer,
    /// Per lane: the texture's (width, height, format) and its bind group.
    textures: Vec<Option<((u32, u32, PixelFormat), wgpu::Texture, wgpu::BindGroup)>>,
}

impl Gpu {
    fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        if std::env::var_os("WGPU_BACKEND").is_none() {
            desc.backends = wgpu::Backends::VULKAN;
        }
        desc.flags = wgpu::InstanceFlags::empty();
        let instance = wgpu::Instance::new(desc);
        let size = window.inner_size();
        let surface = instance.create_surface(window)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("view window"), ..Default::default() }))?;
        let info = surface.display_hdr_info(&adapter);
        tracing::info!("view window: {} ({:?}); display {info:?}", adapter.get_info().name, adapter.get_info().backend);
        let sdr_white = info.luminance.and_then(|l| l.sdr_white_nits).filter(|n| n.is_finite() && *n > 0.0);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("present"), source: wgpu::ShaderSource::Wgsl(PRESENT_WGSL.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("present"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or_else(|| anyhow::anyhow!("the window's surface is not supported by {}", adapter.get_info().name))?;
        let mut gpu = Self { surface, adapter, device, queue, config, encoding: Encoding::Sdr, hdr: true, gain: 1.0, sdr_white, pipelines: vec![], layout, shader, uniform, textures: vec![] };
        gpu.configure(size.width, size.height);
        Ok(gpu)
    }

    /// How it presents: the colour space and, in HDR, the gain.
    fn status(&self) -> String {
        match (self.encoding.hdr(), self.hdr) {
            (true, _) => format!("{}, 1.0 at {:.2}× SDR white", self.encoding.name(), self.gain),
            (false, true) => "SDR (the display offers no HDR surface)".into(),
            (false, false) => "SDR (H: HDR)".into(),
        }
    }

    /// (Re)configures the surface for the window's size and the HDR choice.
    fn configure(&mut self, width: u32, height: u32) {
        let caps = self.surface.get_capabilities(&self.adapter);
        let Some((format, color_space, encoding)) = choose(&caps, self.hdr) else { return };
        if (format, color_space) != (self.config.format, self.config.color_space) || encoding != self.encoding {
            tracing::info!("view window: {format:?}, {color_space:?}");
        }
        self.config.format = format;
        self.config.color_space = color_space;
        self.config.present_mode = wgpu::PresentMode::AutoNoVsync;
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.encoding = encoding;
        self.surface.configure(&self.device, &self.config);
    }

    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return &self.pipelines[i].1;
        }
        let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("present"), bind_group_layouts: &[Some(&self.layout)], immediate_size: 0 });
        let p = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &self.shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(format.into())] }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((format, p));
        &self.pipelines.last().unwrap().1
    }

    /// The lane's texture holding `f` (recreated when its size or format changes).
    fn upload(&mut self, lane: usize, f: &Frame) {
        if lane >= self.textures.len() {
            self.textures.resize_with(lane + 1, || None);
        }
        let key = (f.width, f.height, f.format);
        let (texel, bpp) = match f.format {
            PixelFormat::Rgba8 => (wgpu::TextureFormat::Rgba8Unorm, 4),
            PixelFormat::Rgba16F => (wgpu::TextureFormat::Rgba16Float, 8),
        };
        if f.pixels.len() != (f.width * f.height * bpp) as usize {
            return;
        }
        if self.textures[lane].as_ref().is_none_or(|(k, ..)| *k != key) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("view frame"),
                size: wgpu::Extent3d { width: f.width, height: f.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: texel,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("view frame"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: self.uniform.as_entire_binding() },
                ],
            });
            self.textures[lane] = Some((key, tex, bind));
        }
        let (_, tex, _) = self.textures[lane].as_ref().unwrap();
        self.queue.write_texture(
            tex.as_image_copy(),
            &f.pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(f.width * bpp), rows_per_image: None },
            wgpu::Extent3d { width: f.width, height: f.height, depth_or_array_layers: 1 },
        );
    }

    /// Draws the lanes' textures tiled in a grid, each fit preserving its aspect ratio.
    fn draw(&mut self, frames: &[Option<Frame>], width: u32, height: u32) {
        if (width, height) != (self.config.width, self.config.height) {
            self.configure(width, height);
        }
        let out = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.configure(width, height);
                return;
            }
        };
        let view = out.texture.create_view(&Default::default());
        let encoding = self.encoding;
        let scale = encoding.scale(self.gain, self.sdr_white);
        let mut u = [0u8; 16];
        u[..4].copy_from_slice(&(encoding as u32).to_le_bytes());
        u[4..8].copy_from_slice(&scale.to_le_bytes());
        u[8..12].copy_from_slice(&(GAMMA_22 as u32).to_le_bytes());
        self.queue.write_buffer(&self.uniform, 0, &u);
        // The background, 0x20 grey in the frames' encoding.
        let grey = present_cpu(encoding, 0x20 as f32 / 255.0, scale) as f64;
        let format = self.config.format;
        self.pipeline(format);
        let pipeline = &self.pipelines.iter().find(|(f, _)| *f == format).unwrap().1;
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("view window"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: grey, g: grey, b: grey, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            let n = frames.len().max(1);
            let cols = (n as f64).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            let (tw, th) = (width as usize / cols, height as usize / rows);
            for (i, f) in frames.iter().enumerate() {
                let (Some(f), Some(Some((_, _, bind)))) = (f, self.textures.get(i)) else { continue };
                let (fw, fh) = (f.width as usize, f.height as usize);
                if fw == 0 || fh == 0 || tw == 0 || th == 0 {
                    continue;
                }
                let scale = (tw as f64 / fw as f64).min(th as f64 / fh as f64);
                let (dw, dh) = (((fw as f64 * scale) as usize).max(1), ((fh as f64 * scale) as usize).max(1));
                let ox = (i % cols) * tw + (tw - dw) / 2;
                let oy = (i / cols) * th + (th - dh) / 2;
                pass.set_viewport(ox as f32, oy as f32, dw as f32, dh as f32, 0.0, 1.0);
                pass.set_bind_group(0, bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(out);
    }
}

/// The compositor decodes SDR windows with a pure 2.2 gamma (KWin; and what sRGB panels
/// show), Windows with the piecewise sRGB curve.
const GAMMA_22: bool = !cfg!(windows);

/// `present` of the shader for one grey value (the background's clear colour).
fn present_cpu(e: Encoding, c: f32, scale: f32) -> f32 {
    let decode = |c: f32| if GAMMA_22 { c.powf(2.2) } else { mr_core::lighting::srgb_to_linear_f(c) };
    let encode = |l: f32| if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    match e {
        Encoding::Sdr => c,
        Encoding::SdrSrgbFormat => decode(c),
        Encoding::ExtendedLinear => decode(c) * scale,
        Encoding::Extended => encode(decode(c) * scale),
        Encoding::Pq => {
            let y = (decode(c) * scale).clamp(0.0, 1.0).powf(0.159_301_76);
            ((0.835_937_5 + 18.851_563 * y) / (1.0 + 18.6875 * y)).powf(78.84375)
        }
    }
}

struct App {
    title: String,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    frames: Vec<Option<Frame>>,
    handle: UiHandle,
    mouse_down: bool,
    last_cursor: Option<(f64, f64)>,
    /// The gain last set by `UiEvent::Gain` (the surface's until then).
    gain: f32,
}

impl App {
    fn draw(&mut self) {
        let (Some(window), Some(gpu)) = (&self.window, &mut self.gpu) else { return };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        gpu.draw(&self.frames, size.width, size.height);
    }

    /// H toggles HDR, `[` / `]` change the frames' brightness against SDR white (HDR
    /// surfaces only).
    fn present_key(&mut self, code: KeyCode) {
        let (Some(window), Some(gpu)) = (&self.window, &mut self.gpu) else { return };
        match code {
            KeyCode::KeyH => gpu.hdr = !gpu.hdr,
            KeyCode::BracketLeft => gpu.gain = (gpu.gain / 2f32.powf(0.25)).max(1.0 / 16.0),
            KeyCode::BracketRight => gpu.gain = (gpu.gain * 2f32.powf(0.25)).min(16.0),
            _ => return,
        }
        self.gain = gpu.gain;
        let size = window.inner_size();
        gpu.configure(size.width, size.height);
        tracing::info!("view window: {}", gpu.status());
        let mut input = self.handle.input.lock().unwrap();
        input.status = Some(gpu.status());
        if code != KeyCode::KeyH {
            input.gain = Some(gpu.gain);
        }
        window.request_redraw();
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
        match Gpu::new(window.clone()) {
            Ok(mut g) => {
                g.gain = self.gain;
                self.handle.input.lock().unwrap().status = Some(g.status());
                self.gpu = Some(g);
            }
            Err(e) => tracing::error!("view window: no GPU surface: {e:#}"),
        }
        self.window = Some(window);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UiEvent) {
        match event {
            UiEvent::Frame { lane, width, height, format, pixels } => {
                if lane >= self.frames.len() {
                    self.frames.resize_with(lane + 1, || None);
                }
                let f = Frame { width, height, format, pixels };
                if let Some(g) = &mut self.gpu {
                    g.upload(lane, &f);
                }
                self.frames[lane] = Some(f);
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
            UiEvent::Gain(gain) => {
                self.gain = gain.clamp(1.0 / 16.0, 16.0);
                if let (Some(w), Some(g)) = (&self.window, &mut self.gpu) {
                    g.gain = self.gain;
                    self.handle.input.lock().unwrap().status = Some(g.status());
                    w.request_redraw();
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
                            KeyCode::KeyH | KeyCode::BracketLeft | KeyCode::BracketRight => {
                                drop(input);
                                self.present_key(code);
                            }
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
