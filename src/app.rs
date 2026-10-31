//! Window, input, camera, flight simulation state and the frame composition.
//!
//! The renderer owns GPU resources; this module owns the story: where the ship
//! is, what the camera sees, and how the relativistic state is presented.

use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use glam::{Mat4, Quat, Vec3};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowBuilder};

use crate::clock;
use crate::hud::{self, HudInfo, MenuAction};
use crate::physics::Ship;
use crate::renderer::{FrameData, PostUniform, Renderer, SceneUniform, TrailVertex};
use crate::ship_mesh::ShipVertex;

const FOV_Y: f32 = 58.0;
const NEAR: f32 = 0.0015;
const FAR: f32 = 600.0;
const START_FRAMES: u32 = 90;
/// (render scale, shader geodesic step budget)
const QUALITY: [(f32, f32); 3] = [(0.50, 512.0), (0.70, 512.0), (1.00, 896.0)];

/// Simulation-side state, independent of any window or GPU resource. Keeping it
/// separate lets headless screenshot rendering drive exactly the same flight.
pub struct Sim {
    pub ship: Ship,
    pub journey: crate::journey::Journey,
    pub camera_pos: Vec3,
    pub camera_forward: Vec3,
    pub camera_up: Vec3,
    pub camera_right: Vec3,
    camera_ready: bool,
    pub quality: usize,
    pub paused: bool,
    pub hud_visible: bool,
    pub detailed_hud: bool,
    pub menu_mode: bool,
    pub started: bool,
    pub look_yaw: f32,
    pub look_pitch: f32,
    pub look_held: bool,
    pub fps: f32,
    pub epoch_unix: f64,
    /// Earth seconds elapsed since launch, including the labelled time-lapse.
    /// After the ship crosses the horizon this keeps running on its own so the
    /// calendar visibly leaves the frozen ship clock behind.
    pub earth_elapsed: f64,
    effect_time: f32,
    menu_time: f32,
}

impl Sim {
    pub fn new() -> Self {
        let mut sim = Self {
            ship: Ship::default(),
            journey: crate::journey::Journey::new(),
            camera_pos: Vec3::ZERO,
            camera_forward: Vec3::NEG_Z,
            camera_up: Vec3::Y,
            camera_right: Vec3::X,
            camera_ready: false,
            quality: 1,
            paused: false,
            hud_visible: true,
            detailed_hud: false,
            menu_mode: false,
            started: false,
            look_yaw: 0.0,
            look_pitch: 0.0,
            look_held: false,
            fps: 60.0,
            epoch_unix: clock::unix_now(),
            earth_elapsed: 0.0,
            effect_time: 0.0,
            menu_time: 0.0,
        };
        sim.journey.seek(0.0, &mut sim.ship);
        sim.update_camera(0.0);
        sim
    }

    pub fn reset(&mut self) {
        self.ship.reset();
        self.journey.seek(0.0, &mut self.ship);
        self.started = false;
        self.look_yaw = 0.0;
        self.look_pitch = 0.0;
        self.camera_ready = false;
        self.epoch_unix = clock::unix_now();
        self.earth_elapsed = 0.0;
        self.update_camera(0.0);
    }

    pub fn max_steps(&self) -> f32 {
        QUALITY[self.quality.min(QUALITY.len() - 1)].1
    }

    pub fn render_scale(&self) -> f32 {
        QUALITY[self.quality.min(QUALITY.len() - 1)].0
    }

    pub fn danger(&self) -> f32 {
        self.ship.near_horizon()
    }

    pub fn update(&mut self, dt: f32) {
        if self.menu_mode {
            self.menu_time += dt;
        } else if !self.paused {
            self.seek_journey(self.journey.elapsed + dt as f64);
        }
        self.update_camera(dt);
    }

    pub fn seek_journey(&mut self, time: f64) {
        self.journey.seek(time, &mut self.ship);
        self.effect_time = self.journey.elapsed.min(crate::journey::CLIMAX) as f32;
        self.earth_elapsed = self.ship.far_seconds() * clock::EARTH_TIME_LAPSE;
    }

    fn update_camera(&mut self, _dt: f32) {
        if self.menu_mode {
            let angle = 0.72 + self.menu_time * 0.035;
            self.camera_pos = Vec3::new(angle.cos() * 16.5, 3.0, angle.sin() * 16.5);
        } else {
            self.camera_pos = self.ship.position;
        }
        let forward = -self.camera_pos.normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();
        // Follow the visible rim in the strong field, rather than aiming at
        // a captured direction until the entire image is black. This is camera
        // framing only; rays, observer velocity and trajectory are untouched.
        let radius = self.camera_pos.length();
        let acute = (2.5980762 * (1.0 - 1.0 / radius).sqrt() / radius)
            .clamp(0.0, 1.0)
            .asin();
        let shadow_angle = if radius < 1.5 {
            std::f32::consts::PI - acute
        } else {
            acute
        };
        let rim_turn = if self.menu_mode {
            0.0
        } else {
            (shadow_angle - 0.08).max(0.0)
        };
        let look = Quat::from_axis_angle(up, -self.look_yaw - rim_turn)
            * Quat::from_axis_angle(right, self.look_pitch);
        self.camera_forward = (look * forward).normalize();
        self.camera_up = (look * up).normalize();
        self.camera_right = self.camera_forward.cross(self.camera_up).normalize();
        self.camera_ready = true;
    }

    pub fn scene_uniform(&self, width: u32, height: u32) -> SceneUniform {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let projection = Mat4::perspective_rh(FOV_Y.to_radians(), aspect, NEAR, FAR);
        let view = Mat4::look_to_rh(self.camera_pos, self.camera_forward, self.camera_up);
        let view_proj = projection * view;
        let tan_half = (FOV_Y.to_radians() * 0.5).tan();

        let beta = if self.menu_mode {
            Vec3::ZERO
        } else {
            self.ship.local_velocity()
        };
        SceneUniform {
            cam_velocity: [beta.x, beta.y, beta.z, 0.0],
            view_proj: view_proj.to_cols_array_2d(),
            cam_pos_time: [
                self.camera_pos.x,
                self.camera_pos.y,
                self.camera_pos.z,
                (self.ship.coordinate_time % 1_000_000.0) as f32,
            ],
            cam_right_fov: [
                self.camera_right.x,
                self.camera_right.y,
                self.camera_right.z,
                tan_half,
            ],
            cam_up_aspect: [self.camera_up.x, self.camera_up.y, self.camera_up.z, aspect],
            cam_forward_disk: [
                self.camera_forward.x,
                self.camera_forward.y,
                self.camera_forward.z,
                13.0,
            ],
            viewport_disk: [width as f32, height as f32, 3.0, 1.0],
            ship_pos: [
                self.ship.position.x,
                self.ship.position.y,
                self.ship.position.z,
                self.ship.lost as u8 as f32,
            ],
            misc: [
                self.danger(),
                self.max_steps(),
                self.effect_time,
                self.quality as f32,
            ],
        }
    }

    pub fn post_uniform(&self, width: u32, height: u32) -> PostUniform {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let exposure = if self.menu_mode { 1.55 } else { 1.05 };
        PostUniform {
            params: [0.18, exposure, self.effect_time, self.effect_time * 1.37],
            screen: [width as f32, height as f32, self.danger(), aspect],
        }
    }

    pub fn ship_vertices(&self, out: &mut Vec<ShipVertex>) {
        out.clear();
        if !self.menu_mode {
            crate::space_text::append(
                out,
                self.camera_pos,
                self.camera_forward,
                self.journey.elapsed,
            );
        }
    }

    pub fn effect_vertices(&self, out: &mut Vec<TrailVertex>) {
        out.clear();
    }
}

struct HudGpu {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
}

impl HudGpu {
    fn new(ctx: egui::Context, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let renderer = egui_wgpu::Renderer::new(device, format, None, 1);
        Self { ctx, renderer }
    }
}

pub struct App {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    sim: Sim,
    hud: HudGpu,
    egui_state: egui_winit::State,
    keys: HashSet<KeyCode>,
    mouse_left: bool,
    mouse_right: bool,
    last_cursor: Option<(f64, f64)>,
    ship_vertices: Vec<ShipVertex>,
    trail_vertices: Vec<TrailVertex>,
    last_frame: Instant,
    pub should_exit: bool,
    music: crate::music::Music,
    resolution_timer: Instant,
}

impl App {
    fn new(event_loop: &EventLoopWindowTarget<()>) -> Result<Self, Box<dyn Error>> {
        let attributes = WindowBuilder::new()
            .with_title("GARGANTUA - A Scripted Descent")
            .with_inner_size(LogicalSize::new(1400.0, 900.0))
            .with_min_inner_size(LogicalSize::new(960.0, 640.0));
        let window = Arc::new(attributes.build(event_loop)?);

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("no compatible GPU adapter found")?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("gargantua device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut sim = Sim::new();
        sim.menu_mode = true;
        sim.paused = true;
        let renderer = Renderer::new(&device, &queue, format, width, height, sim.render_scale());

        let egui_ctx = egui::Context::default();
        hud::configure_style(&egui_ctx);
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &*window,
            Some(window.scale_factor() as f32),
            Some(adapter.limits().max_texture_dimension_2d as usize),
        );
        let hud = HudGpu::new(egui_ctx, &device, format);

        log::info!(
            "GPU: {} | {:?} | surface {:?} {}x{}",
            adapter.get_info().name,
            adapter.get_info().backend,
            format,
            width,
            height
        );

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            sim,
            hud,
            egui_state,
            keys: HashSet::new(),
            mouse_left: false,
            mouse_right: false,
            last_cursor: None,
            ship_vertices: Vec::new(),
            trail_vertices: Vec::new(),
            last_frame: Instant::now(),
            should_exit: false,
            music: crate::music::Music::new(),
            resolution_timer: Instant::now(),
        })
    }

    fn set_quality(&mut self, index: usize) {
        self.sim.quality = index.min(QUALITY.len() - 1);
        let scale = self.sim.render_scale();
        self.renderer
            .set_render_scale(&self.device, &self.queue, scale);
    }

    fn start_flight(&mut self) {
        if !self.sim.started {
            self.music.start();
        }
        self.music.set_paused(false);
        self.sim.started = true;
        self.sim.menu_mode = false;
        self.sim.paused = false;
        self.keys.clear();
        self.mouse_left = false;
        self.mouse_right = false;
    }

    fn toggle_menu(&mut self) {
        if self.sim.menu_mode {
            self.start_flight();
        } else {
            self.sim.menu_mode = true;
            self.sim.paused = true;
            self.music.set_paused(true);
            self.sim.look_held = false;
            self.keys.clear();
            self.mouse_left = false;
            self.mouse_right = false;
        }
    }

    fn apply_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::None => {}
            MenuAction::Play => self.start_flight(),
            MenuAction::Restart => {
                self.sim.reset();
                self.start_flight();
            }
            MenuAction::Quit => self.should_exit = true,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer
            .resize(&self.device, &self.queue, width, height);
    }

    fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        match event.state {
            ElementState::Pressed => {
                self.keys.insert(code);
                if event.repeat {
                    return;
                }
                if code == KeyCode::Escape {
                    self.toggle_menu();
                    return;
                }
                if self.sim.menu_mode {
                    return;
                }
                match code {
                    KeyCode::KeyX | KeyCode::KeyT => {
                        self.sim.look_yaw = 0.0;
                        self.sim.look_pitch = 0.0;
                    }
                    KeyCode::KeyP | KeyCode::KeyM | KeyCode::Space => {
                        self.sim.paused = !self.sim.paused;
                        self.music.set_paused(self.sim.paused);
                    }
                    KeyCode::KeyR => {
                        self.sim.reset();
                        self.start_flight();
                    }
                    KeyCode::KeyI => self.sim.detailed_hud = !self.sim.detailed_hud,
                    KeyCode::KeyH => self.sim.hud_visible = !self.sim.hud_visible,
                    KeyCode::Digit1 => self.set_quality(0),
                    KeyCode::Digit2 => self.set_quality(1),
                    KeyCode::Digit3 => self.set_quality(2),
                    _ => {}
                }
            }
            ElementState::Released => {
                self.keys.remove(&code);
            }
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = self.window.inner_size();
                self.resize(size.width, size.height);
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.mouse_left = false;
                self.mouse_right = false;
                self.sim.look_held = false;
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(event),
            WindowEvent::CursorMoved { position, .. } => {
                if !self.sim.menu_mode {
                    let scale = self.window.scale_factor().max(0.5);
                    if let Some((last_x, last_y)) = self.last_cursor {
                        let dx = ((position.x - last_x) / scale) as f32;
                        let dy = ((position.y - last_y) / scale) as f32;
                        if self.mouse_left || self.mouse_right {
                            self.sim.look_yaw =
                                (self.sim.look_yaw + dx * 0.0045).rem_euclid(std::f32::consts::TAU);
                            self.sim.look_pitch =
                                (self.sim.look_pitch - dy * 0.0045).clamp(-1.5, 1.5);
                        }
                    }
                }
                self.last_cursor = Some((position.x, position.y));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = *state == ElementState::Pressed;
                match button {
                    winit::event::MouseButton::Left => {
                        self.mouse_left = down;
                        self.sim.look_held = down && !self.sim.menu_mode;
                    }
                    winit::event::MouseButton::Right => self.mouse_right = down,
                    _ => {}
                }
                if down {
                    // Start a fresh drag from wherever the pointer already is.
                    self.last_cursor = None;
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }

        let window = self.window.clone();
        let _ = self.egui_state.on_window_event(&window, event);
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    fn redraw(&mut self) {
        self.music.update();
        let physical = self.window.inner_size();
        let native_scale = self.window.scale_factor() as f32;
        let zoom = ((physical.width as f32 / native_scale / 1400.0)
            .min(physical.height as f32 / native_scale / 900.0))
        .clamp(0.5, 1.0);
        self.hud.ctx.set_zoom_factor(zoom);
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        if dt > 0.0 {
            let instant = (1.0 / dt).min(1000.0);
            self.sim.fps = self.sim.fps * 0.9 + instant * 0.1;
        }

        self.music
            .set_paused(self.sim.menu_mode || self.sim.paused || !self.sim.started);
        if self.sim.started && !self.sim.menu_mode {
            self.sim.seek_journey(self.music.elapsed());
            let axis = |positive, negative| {
                (self.keys.contains(&positive) as u8 as f32)
                    - (self.keys.contains(&negative) as u8 as f32)
            };
            self.sim.look_yaw += axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) * dt;
            self.sim.look_pitch = (self.sim.look_pitch
                + axis(KeyCode::ArrowUp, KeyCode::ArrowDown) * dt)
                .clamp(-1.5, 1.5);
            self.sim.update_camera(dt);
        } else {
            self.sim.update(dt);
        }

        if !self.sim.menu_mode && self.resolution_timer.elapsed().as_secs_f32() > 2.0 {
            let current = self.renderer.render_scale;
            let ceiling = self.sim.render_scale();
            let next = if self.sim.fps < 48.0 {
                (current * 0.90).max(0.45)
            } else if self.sim.fps > 58.0 {
                (current * 1.04).min(ceiling)
            } else {
                current
            };
            if (next - current).abs() > 0.005 {
                self.renderer
                    .set_render_scale(&self.device, &self.queue, next);
            }
            self.resolution_timer = Instant::now();
        }
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("surface out of memory");
                self.should_exit = true;
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });

        let width = self.config.width;
        let height = self.config.height;
        self.sim.ship_vertices(&mut self.ship_vertices);
        self.sim.effect_vertices(&mut self.trail_vertices);
        let scene = self.sim.scene_uniform(width, height);
        let post = self.sim.post_uniform(width, height);
        let frame_data = FrameData {
            scene,
            post,
            ship_vertices: &self.ship_vertices,
            trail_vertices: &self.trail_vertices,
        };
        self.renderer
            .encode_scene(&self.device, &self.queue, &mut encoder, &view, &frame_data);

        let mut command_buffers: Vec<wgpu::CommandBuffer> = Vec::new();
        if self.sim.menu_mode || self.sim.hud_visible {
            let window = self.window.clone();
            let raw_input = self.egui_state.take_egui_input(&window);
            let menu_mode = self.sim.menu_mode;
            let can_resume = self.sim.started;
            let fps = self.sim.fps;
            let render_scale = self.renderer.render_scale;
            let mut action = MenuAction::None;
            let full = {
                let sim = &self.sim;
                self.hud.ctx.run(raw_input, |ctx| {
                    if menu_mode {
                        action = hud::draw_menu(ctx, can_resume);
                    } else {
                        let info = sim.hud_info(fps, render_scale);
                        hud::draw(ctx, &info);
                    }
                })
            };
            self.egui_state
                .handle_platform_output(&window, full.platform_output);
            self.apply_menu_action(action);

            let ppp = full.pixels_per_point;
            let jobs = self.hud.ctx.tessellate(full.shapes, ppp);
            command_buffers = paint_hud(
                &mut self.hud.renderer,
                &self.device,
                &self.queue,
                &mut encoder,
                &view,
                &jobs,
                &full.textures_delta,
                ppp,
                (width, height),
            );
        }

        self.queue.submit(
            command_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        frame.present();
    }
}

impl Sim {
    pub fn hud_info(&self, fps: f32, render_scale: f32) -> HudInfo<'_> {
        let target = -self.camera_pos.normalize();
        HudInfo {
            target_view: [
                target.dot(self.camera_right),
                target.dot(self.camera_up),
                target.dot(self.camera_forward),
            ],
            ship: &self.ship,
            paused: self.paused,
            journey_time: self.journey.elapsed,
            detailed: self.detailed_hud,
            fps,
            render_scale,
            epoch_unix: self.epoch_unix,
            earth_elapsed: self.earth_elapsed,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_hud(
    egui_renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    paint_jobs: &[egui::ClippedPrimitive],
    textures: &egui::TexturesDelta,
    pixels_per_point: f32,
    size: (u32, u32),
) -> Vec<wgpu::CommandBuffer> {
    for (id, delta) in &textures.set {
        egui_renderer.update_texture(device, queue, *id, delta);
    }

    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [size.0, size.1],
        pixels_per_point,
    };
    let command_buffers = egui_renderer.update_buffers(device, queue, encoder, paint_jobs, &screen);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        egui_renderer.render(&mut pass, paint_jobs, &screen);
    }

    for id in &textures.free {
        egui_renderer.free_texture(id);
    }

    command_buffers
}

/// Run the interactive native window.
pub fn run_native() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app: Option<App> = None;

    event_loop.run(move |event, active| match event {
        winit::event::Event::Resumed => {
            if app.is_none() {
                match App::new(active) {
                    Ok(a) => app = Some(a),
                    Err(err) => {
                        log::error!("failed to create application: {err}");
                        active.exit();
                    }
                }
            }
        }
        winit::event::Event::WindowEvent { event, .. } => {
            if let Some(app) = app.as_mut() {
                match event {
                    WindowEvent::CloseRequested => active.exit(),
                    WindowEvent::RedrawRequested => app.redraw(),
                    other => app.window_event(&other),
                }
                if app.should_exit {
                    active.exit();
                }
            }
        }
        winit::event::Event::AboutToWait => {
            if let Some(app) = app.as_mut() {
                app.request_redraw();
            }
        }
        _ => {}
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Headless screenshot rendering
// ---------------------------------------------------------------------------

pub struct ScreenshotOptions {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub horizon: bool,
    pub dive: bool,
    pub menu: bool,
    pub orbit: bool,
    pub hud: bool,
    pub at: Option<f64>,
    pub benchmark: bool,
}

pub fn render_screenshot(options: &ScreenshotOptions) -> Result<(), Box<dyn Error>> {
    let width = options.width.max(320);
    let height = options.height.max(240);
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok_or("no compatible GPU adapter found")?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("gargantua screenshot device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))?;

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut sim = Sim::new();
    sim.quality = 2;
    sim.fps = 60.0;
    sim.hud_visible = options.hud;
    if options.menu {
        sim.menu_mode = true;
        sim.paused = true;
        sim.started = false;
    } else {
        let seconds = options.at.unwrap_or_else(|| {
            if options.dive {
                230.0
            } else if options.horizon {
                223.0
            } else if options.orbit {
                180.0
            } else {
                options.frames as f64 / 60.0
            }
        });
        sim.seek_journey(seconds);
        sim.update_camera(0.0);
    }
    sim.update_camera(0.0);
    let frames = options.frames;
    sim.fps = 60.0;

    let render_scale = if options.benchmark { 0.70 } else { 1.0 };
    let mut renderer = Renderer::new(&device, &queue, format, width, height, render_scale);
    let mut hud_gpu = HudGpu::new(egui::Context::default(), &device, format);
    hud::configure_style(&hud_gpu.ctx);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screenshot target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut ship_vertices = Vec::new();
    let mut trail_vertices = Vec::new();
    sim.ship_vertices(&mut ship_vertices);
    sim.effect_vertices(&mut trail_vertices);

    if options.benchmark {
        let data = FrameData {
            scene: sim.scene_uniform(width, height),
            post: sim.post_uniform(width, height),
            ship_vertices: &ship_vertices,
            trail_vertices: &trail_vertices,
        };
        let mut start = Instant::now();
        for i in 0..24 {
            if i == 4 {
                start = Instant::now();
            }
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.encode_scene(&device, &queue, &mut encoder, &view, &data);
            queue.submit(Some(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);
        }
        let seconds = start.elapsed().as_secs_f64();
        println!(
            "Scene benchmark at {:.1}s: {:.1} FPS, {:.2} ms/frame, {}x{} at 70% render",
            sim.journey.elapsed,
            20.0 / seconds,
            seconds * 1000.0 / 20.0,
            width,
            height
        );
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("screenshot encoder"),
    });
    let frame_data = FrameData {
        scene: sim.scene_uniform(width, height),
        post: sim.post_uniform(width, height),
        ship_vertices: &ship_vertices,
        trail_vertices: &trail_vertices,
    };
    renderer.encode_scene(&device, &queue, &mut encoder, &view, &frame_data);

    let mut command_buffers = Vec::new();
    let menu_mode = sim.menu_mode;
    if menu_mode || sim.hud_visible {
        let ppp = (width as f32 / 1400.0).min(height as f32 / 900.0);
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32 / ppp, height as f32 / ppp),
            )),
            ..Default::default()
        };
        raw.viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root viewport")
            .native_pixels_per_point = Some(ppp);
        raw.max_texture_side = Some(adapter.limits().max_texture_dimension_2d as usize);
        raw.time = Some(if menu_mode {
            sim.menu_time as f64
        } else {
            sim.effect_time as f64
        });
        let info = sim.hud_info(60.0, 1.0);
        let draw = |ctx: &egui::Context| {
            if menu_mode {
                let _ = hud::draw_menu(ctx, false);
            } else {
                hud::draw(ctx, &info);
            }
        };
        // Newly created anchor-based `Area`s do not know their size until they
        // have been laid out once. Run the HUD a few times so every panel gets
        // its final size before the primitives that will be painted are
        // produced.
        for _ in 0..2 {
            let pass = hud_gpu.ctx.run(raw.clone(), draw);
            for (id, delta) in &pass.textures_delta.set {
                hud_gpu.renderer.update_texture(&device, &queue, *id, delta);
            }
        }
        let full = hud_gpu.ctx.run(raw, draw);
        let ppp = full.pixels_per_point;
        let jobs = hud_gpu.ctx.tessellate(full.shapes, ppp);
        command_buffers = paint_hud(
            &mut hud_gpu.renderer,
            &device,
            &queue,
            &mut encoder,
            &view,
            &jobs,
            &full.textures_delta,
            ppp,
            (width, height),
        );
    }

    // Copy the image to a mappable buffer, accounting for wgpu's 256-byte row
    // alignment requirement.
    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let output_buffer_size = (padded_bytes_per_row as u64) * (height as u64);
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot readback"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(
        command_buffers
            .into_iter()
            .chain(std::iter::once(encoder.finish())),
    );
    device.poll(wgpu::Maintain::Wait);

    let slice = output_buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    receiver.recv()??;

    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    {
        let data = slice.get_mapped_range();
        for row in 0..height {
            let start = (row as usize) * (padded_bytes_per_row as usize);
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }
    }
    output_buffer.unmap();

    image::save_buffer(
        &options.path,
        &pixels,
        width,
        height,
        image::ColorType::Rgba8,
    )?;
    log::info!(
        "saved {} ({}x{}, {} frames simulated)",
        options.path,
        width,
        height,
        frames
    );
    println!(
        "saved {} ({}x{}, {} frames simulated)",
        options.path, width, height, frames
    );
    Ok(())
}

/// Tiny helper shared by the CLI.
pub fn parse_screenshot_args(args: &[String]) -> Option<ScreenshotOptions> {
    let mut path = None;
    let mut width = 1920u32;
    let mut height = 1080u32;
    let mut frames = START_FRAMES;
    let mut horizon = false;
    let mut dive = false;
    let mut menu = false;
    let mut orbit = false;
    let mut hud = true;
    let mut at = None;
    let mut benchmark = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" | "-o" => {
                i += 1;
                path = args.get(i).cloned();
            }
            "--width" => {
                i += 1;
                width = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(width);
            }
            "--height" => {
                i += 1;
                height = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(height);
            }
            "--frames" => {
                i += 1;
                frames = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(frames);
            }
            "--benchmark" => {
                benchmark = true;
                if path.is_none() {
                    path = Some("/tmp/gargantua-benchmark.png".into());
                }
            }
            "--at" => {
                i += 1;
                at = args.get(i).and_then(|v| v.parse().ok());
            }
            "--descent" => {}
            "--horizon" => horizon = true,
            "--dive" => dive = true,
            "--menu" => menu = true,
            "--orbit" => orbit = true,
            "--no-hud" => hud = false,
            _ => {}
        }
        i += 1;
    }
    path.map(|path| ScreenshotOptions {
        path,
        width,
        height,
        frames,
        horizon,
        dive,
        menu,
        orbit,
        hud,
        at,
        benchmark,
    })
}

#[cfg(test)]
mod scripted_tests {
    use super::*;
    #[test]
    fn free_look_changes_only_the_view_even_after_the_ending() {
        let mut sim = Sim::new();
        sim.seek_journey(180.0);
        sim.update_camera(0.0);
        let position = sim.ship.position;
        let clock = sim.ship.proper_time;
        let initial_view = sim.camera_forward;
        sim.look_yaw = 1.0;
        sim.look_pitch = 0.4;
        sim.update_camera(0.0);
        assert!(sim.camera_forward.dot(initial_view) < 0.8);
        assert_eq!(sim.ship.position, position);
        assert_eq!(sim.ship.proper_time, clock);
        sim.seek_journey(240.0);
        let position = sim.ship.position;
        sim.look_yaw += 1.0;
        sim.update_camera(0.0);
        assert_eq!(sim.ship.position, position);
        assert!(sim.ship.lost);
    }
    #[test]
    fn paused_simulation_preserves_timeline_and_free_look_does_not_spring_back() {
        let mut sim = Sim::new();
        sim.seek_journey(90.0);
        sim.look_yaw = 1.2;
        sim.paused = true;
        sim.update(10.0);
        assert_eq!(sim.journey.elapsed, 90.0);
        assert_eq!(sim.look_yaw, 1.2);
    }
}
