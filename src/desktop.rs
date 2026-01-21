//! Reactive CPU rasterization with retained bitmap presentation (Win32 GDI on Windows).
use crate::ui::App;
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use serde::Serialize;
use std::{
    num::NonZeroU32,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use website_blocker::store::{Store, atomic_write};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle},
    window::{Window, WindowId},
};

#[derive(Debug)]
pub enum Wake {
    Repaint { at: Instant, pass: u64 },
}

#[derive(Default, Clone, Serialize)]
struct Counters {
    layouts: u64,
    rasters: u64,
    presents: u64,
    cached_blits: u64,
    hidden_redraws_skipped: u64,
    layout_max_ms: f64,
    raster_max_ms: f64,
    present_max_ms: f64,
}

#[derive(Serialize)]
struct Sample {
    state: &'static str,
    elapsed_seconds: f64,
    cpu_seconds: f64,
    counters: Counters,
    window_visible: Option<bool>,
    window_minimized: Option<bool>,
}

#[derive(Serialize)]
struct Report {
    backend: &'static str,
    pid: u32,
    executable: PathBuf,
    logical_cores: usize,
    loaded_graphics_modules: Vec<String>,
    samples: Vec<Sample>,
    counters: Counters,
}

struct View {
    surface: softbuffer::Surface<OwnedDisplayHandle, Rc<Window>>,
    window: Rc<Window>,
    input: egui_winit::State,
    viewport: egui::ViewportInfo,
    renderer: EguiSoftwareRender,
    bitmap: Vec<[u8; 4]>,
    size: [u32; 2],
    hidden: bool,
}

struct Host {
    app: App,
    ctx: egui::Context,
    view: Option<View>,
    dirty: bool,
    repaint: Option<Instant>,
    start: Instant,
    error: Option<String>,
    capture: Option<PathBuf>,
    capture_frames: u8,
    diagnostic: Option<PathBuf>,
    phase: usize,
    report: Report,
}

const PHASES: &[(u64, &str)] = &[
    (2500, "settled_start"),
    (7500, "visible_idle_end"),
    (9500, "minimized_end"),
    (11500, "restored_settled"),
    (14500, "restored_idle_end"),
    (16500, "hidden_end"),
];

pub fn run(store: Store) -> Result<(), String> {
    let capture = std::env::var("WEBSITEBLOCKER_CAPTURE")
        .ok()
        .map(|value| {
            let (screen, path) = value
                .split_once('|')
                .ok_or("Capture expects screen|absolute.png.")?;
            if !["focus", "rules", "settings", "browser"].contains(&screen)
                || !PathBuf::from(path).is_absolute()
                || !path.ends_with(".png")
            {
                return Err(
                    "Choose focus, rules or settings and an absolute PNG output path.".to_string(),
                );
            }
            Ok(PathBuf::from(path))
        })
        .transpose()?;
    let diagnostic = std::env::var_os("WEBSITEBLOCKER_DIAGNOSTICS").map(PathBuf::from);
    if (capture.is_some() || diagnostic.is_some())
        && std::env::var_os("WEBSITEBLOCKER_DATA_DIR").is_none()
    {
        return Err(
            "Diagnostic launches require an explicit isolated WEBSITEBLOCKER_DATA_DIR.".into(),
        );
    }
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    let ctx = egui::Context::default();
    let proxy = event_loop.create_proxy();
    ctx.set_request_repaint_callback(move |info| {
        if let Some(at) = Instant::now().checked_add(info.delay) {
            let _ = proxy.send_event(Wake::Repaint {
                at,
                pass: info.current_cumulative_pass_nr,
            });
        }
    });
    let app = App::with_context(&ctx, store);
    let mut host = Host {
        app,
        ctx,
        view: None,
        dirty: true,
        repaint: None,
        start: Instant::now(),
        error: None,
        capture,
        capture_frames: 0,
        diagnostic,
        phase: 0,
        report: Report {
            backend: "egui CPU software rasterizer -> softbuffer Win32 GDI",
            pid: std::process::id(),
            executable: std::env::current_exe().map_err(|e| e.to_string())?,
            logical_cores: std::thread::available_parallelism().map_or(1, |n| n.get()),
            loaded_graphics_modules: vec![],
            samples: vec![],
            counters: Counters::default(),
        },
    };
    trace(&host.diagnostic, "enter event loop");
    event_loop.run_app(&mut host).map_err(|e| e.to_string())?;
    host.report.loaded_graphics_modules = loaded_graphics_modules();
    if let Some(path) = &host.diagnostic {
        atomic_write(
            path,
            &serde_json::to_vec_pretty(&host.report).map_err(|e| e.to_string())?,
        )?;
    }
    host.error.map_or(Ok(()), Err)
}

impl Host {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));
    }
    fn visible(&self) -> bool {
        self.view.as_ref().is_some_and(|v| {
            !v.hidden && v.size[0] > 0 && v.size[1] > 0 && !v.window.is_minimized().unwrap_or(false)
        })
    }
    fn invalidate(&mut self) {
        self.dirty = true;
        if self.visible()
            && let Some(view) = &self.view
        {
            view.window.request_redraw();
        }
    }
    fn present(&mut self) -> Result<(), String> {
        let view = self.view.as_mut().ok_or("No window")?;
        let started = Instant::now();
        let mut buffer = view.surface.buffer_mut().map_err(|e| e.to_string())?;
        for (destination, pixel) in buffer.iter_mut().zip(&view.bitmap) {
            *destination = u32::from_le_bytes(*pixel) & 0x00ff_ffff;
        }
        buffer.present().map_err(|e| e.to_string())?;
        self.report.counters.presents += 1;
        self.report.counters.present_max_ms = self
            .report
            .counters
            .present_max_ms
            .max(started.elapsed().as_secs_f64() * 1000.0);
        Ok(())
    }
    fn draw(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        if !self.visible() {
            self.report.counters.hidden_redraws_skipped += 1;
            return Ok(());
        }
        if !self.dirty && self.capture.is_none() {
            self.report.counters.cached_blits += 1;
            return self.present();
        }
        if self.report.counters.layouts < 2 {
            trace(&self.diagnostic, "draw: begin layout");
        }
        self.dirty = false;
        self.repaint = None;
        let view = self.view.as_mut().ok_or("No window")?;
        egui_winit::update_viewport_info(&mut view.viewport, &self.ctx, &view.window, false);
        let mut input = view.input.take_egui_input(&view.window);
        input
            .viewports
            .insert(egui::ViewportId::ROOT, view.viewport.clone());
        let started = Instant::now();
        let output = self.ctx.run(input, |ctx| self.app.render(ctx));
        if self.report.counters.layouts < 2 {
            trace(&self.diagnostic, "draw: layout complete");
        }
        self.report.counters.layouts += 1;
        self.report.counters.layout_max_ms = self
            .report
            .counters
            .layout_max_ms
            .max(started.elapsed().as_secs_f64() * 1000.0);
        view.input
            .handle_platform_output(&view.window, output.platform_output);
        let mut actions = vec![];
        for (_, viewport) in output.viewport_output {
            egui_winit::process_viewport_commands(
                &self.ctx,
                &mut view.viewport,
                viewport.commands,
                &view.window,
                &mut actions,
            );
        }
        if self.app.close_requested {
            event_loop.exit();
            return Ok(());
        }
        let [width, height] = view.size;
        view.bitmap
            .resize(width as usize * height as usize, [243, 246, 244, 255]);
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let raster_start = Instant::now();
        view.renderer.render(
            &mut BufferMutRef::new(&mut view.bitmap, width as usize, height as usize),
            &primitives,
            &output.textures_delta,
            output.pixels_per_point,
        );
        if self.report.counters.rasters < 2 {
            trace(&self.diagnostic, "draw: raster complete");
        }
        self.report.counters.rasters += 1;
        self.report.counters.raster_max_ms = self
            .report
            .counters
            .raster_max_ms
            .max(raster_start.elapsed().as_secs_f64() * 1000.0);
        self.present()?;
        if self.report.counters.presents < 3 {
            trace(&self.diagnostic, "draw: present complete");
        }
        if let Some(path) = &self.capture
            && self.app.capture_ready()
        {
            self.capture_frames += 1;
            if self.capture_frames >= 3 {
                let view = self.view.as_ref().unwrap();
                let rgba: Vec<_> = view
                    .bitmap
                    .iter()
                    .flat_map(|p| [p[2], p[1], p[0], p[3]])
                    .collect();
                let pixels = image::RgbaImage::from_raw(width, height, rgba)
                    .ok_or("Invalid capture dimensions.")?;
                let mut encoded = std::io::Cursor::new(Vec::new());
                pixels
                    .write_to(&mut encoded, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                atomic_write(path, encoded.get_ref())?;
                event_loop.exit();
            } else {
                self.repaint = Some(Instant::now());
            }
        }
        Ok(())
    }
    fn diagnostic_tick(&mut self, event_loop: &ActiveEventLoop) {
        if self.diagnostic.is_none() || self.phase >= PHASES.len() {
            return;
        }
        let (at, state) = PHASES[self.phase];
        if self.start.elapsed() < Duration::from_millis(at) {
            return;
        }
        self.report.samples.push(Sample {
            state,
            elapsed_seconds: self.start.elapsed().as_secs_f64(),
            cpu_seconds: cpu_seconds(),
            counters: self.report.counters.clone(),
            window_visible: self.view.as_ref().and_then(|v| v.window.is_visible()),
            window_minimized: self.view.as_ref().and_then(|v| v.window.is_minimized()),
        });
        if let Some(path) = &self.diagnostic {
            let _ = atomic_write(
                path,
                &serde_json::to_vec_pretty(&self.report).unwrap_or_default(),
            );
            trace(&self.diagnostic, state);
        }
        if let Some(view) = &mut self.view {
            match self.phase {
                1 => {
                    view.window.set_minimized(true);
                    view.hidden = true;
                    self.repaint = None;
                }
                2 => {
                    view.window.set_minimized(false);
                    view.hidden = false;
                    self.dirty = true;
                    view.window.request_redraw();
                }
                4 => {
                    view.window.set_visible(false);
                    view.hidden = true;
                    self.repaint = None;
                }
                5 => event_loop.exit(),
                _ => {}
            }
        }
        self.phase += 1;
    }
}

impl ApplicationHandler<Wake> for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.view.is_some() {
            return;
        }
        trace(&self.diagnostic, "resumed: create window");
        let result = (|| -> Result<View, String> {
            let window = Rc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("WebsiteBlocker")
                            .with_active(!self.app.diagnostic)
                            .with_inner_size(winit::dpi::LogicalSize::new(880.0, 680.0))
                            .with_min_inner_size(winit::dpi::LogicalSize::new(740.0, 580.0)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            trace(&self.diagnostic, "window created: create GDI surface");
            let context = softbuffer::Context::new(event_loop.owned_display_handle())
                .map_err(|e| e.to_string())?;
            let mut surface =
                softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
            let size = window.inner_size();
            surface
                .resize(
                    NonZeroU32::new(size.width.max(1)).unwrap(),
                    NonZeroU32::new(size.height.max(1)).unwrap(),
                )
                .map_err(|e| e.to_string())?;
            trace(&self.diagnostic, "surface created: initialize input");
            let input = egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                None,
                None,
            );
            Ok(View {
                window,
                surface,
                input,
                viewport: egui::ViewportInfo::default(),
                renderer: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
                bitmap: vec![],
                size: [size.width, size.height],
                hidden: false,
            })
        })();
        match result {
            Ok(view) => {
                self.view = Some(view);
                self.invalidate();
            }
            Err(error) => self.fail(event_loop, error),
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wake) {
        self.app.receive(&self.ctx);
        if self.app.close_requested {
            event_loop.exit();
            return;
        }
        let Wake::Repaint { at, pass } = event;
        if self.ctx.cumulative_pass_nr() <= pass + 1 {
            if self.visible() {
                self.repaint = Some(self.repaint.map_or(at, |old| old.min(at)));
            } else {
                self.dirty = true;
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        if id != view.window.id() {
            return;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
            return;
        }
        if matches!(event, WindowEvent::RedrawRequested) {
            if let Err(error) = self.draw(event_loop) {
                self.fail(event_loop, error);
            }
            return;
        }
        match &event {
            WindowEvent::Occluded(value) => {
                view.hidden = *value;
                if *value {
                    self.repaint = None;
                }
            }
            WindowEvent::Resized(size) => {
                view.size = [size.width, size.height];
                view.hidden = size.width == 0
                    || size.height == 0
                    || view.window.is_minimized().unwrap_or(false);
                if !view.hidden
                    && let Err(error) = view.surface.resize(
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    )
                {
                    self.fail(event_loop, error);
                    return;
                }
            }
            _ => {}
        }
        if view.input.on_window_event(&view.window, &event).repaint {
            self.invalidate();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.diagnostic_tick(event_loop);
        // winit 0.30's Win32 loop waits immediately after AboutToWait, even when
        // exit was requested in that callback. Expire this one wait so it can quit.
        if event_loop.exiting() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));
            return;
        }
        if self.capture.is_some()
            && (self.start.elapsed() > Duration::from_secs(10) || self.report.counters.layouts > 24)
        {
            self.fail(
                event_loop,
                "Capture did not become ready within its frame/time limit.",
            );
            return;
        }
        let now = Instant::now();
        if self.visible() {
            self.app.maintenance_tick();
            if self.repaint.is_some_and(|at| at <= now) {
                self.repaint = None;
                self.invalidate();
            }
        } else {
            self.repaint = None;
        }
        let diagnostic = self
            .diagnostic
            .as_ref()
            .and_then(|_| PHASES.get(self.phase))
            .map(|(ms, _)| self.start + Duration::from_millis(*ms));
        let capture_limit = self
            .capture
            .as_ref()
            .map(|_| self.start + Duration::from_secs(10));
        let maintenance = self
            .visible()
            .then(|| self.app.maintenance_deadline())
            .flatten();
        let next = [self.repaint, maintenance, diagnostic, capture_limit]
            .into_iter()
            .flatten()
            .min();
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        trace(&self.diagnostic, "release window and GDI surface");
        self.ctx.set_request_repaint_callback(|_| {});
        self.view = None;
        trace(&self.diagnostic, "window resources released");
    }
}

#[cfg(windows)]
fn cpu_seconds() -> f64 {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentProcess, GetProcessTimes},
    };
    let mut times = [FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    }; 4];
    // SAFETY: The pseudo-handle refers to this process and output pointers are valid.
    let measured = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut times[0],
            &mut times[1],
            &mut times[2],
            &mut times[3],
        )
    };
    if measured == 0 {
        return -1.0;
    }
    times[2..]
        .iter()
        .map(|v| {
            ((u64::from(v.dwHighDateTime) << 32) | u64::from(v.dwLowDateTime)) as f64 / 10_000_000.0
        })
        .sum()
}
#[cfg(not(windows))]
fn cpu_seconds() -> f64 {
    0.0
}

#[cfg(windows)]
fn loaded_graphics_modules() -> Vec<String> {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    [
        "gdi32.dll",
        "opengl32.dll",
        "d3d11.dll",
        "d3d12.dll",
        "dxgi.dll",
        "vulkan-1.dll",
    ]
    .into_iter()
    .filter_map(|name| {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: This only checks whether an existing module is loaded; it never loads one.
        (!unsafe { GetModuleHandleW(wide.as_ptr()) }.is_null()).then(|| name.to_string())
    })
    .collect()
}
#[cfg(not(windows))]
fn loaded_graphics_modules() -> Vec<String> {
    vec![]
}

fn trace(path: &Option<PathBuf>, label: &str) {
    if let Some(path) = path {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.with_extension("progress.txt"))
        {
            let _ = writeln!(file, "{label}");
        }
    }
}
