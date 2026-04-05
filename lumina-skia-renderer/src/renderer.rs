use crate::config::WindowConfig;
use crate::core::{AssetManager, AudioPlayer, Painter};
use crate::screens::{ingame::GameScreen, Screen, ScreenTransition};
use crate::ui::UiDrawer;
use crate::vk_utils::context::VulkanRenderContext;
use crate::vk_utils::renderer::VulkanRenderer;

use lumina_core::config::SystemConfig;
use lumina_core::manager::ScriptManager;
use lumina_core::Ctx;
use lumina_shared;
use lumina_ui::{
    input::UiContext,
    Rect
};
use skia_safe::textlayout::{FontCollection, TypefaceFontProvider};
use skia_safe::{FontMgr, RuntimeEffect};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId}
};

// 设计分辨率
const DESIGN_WIDTH: f32 = 1920.0;
const DESIGN_HEIGHT: f32 = 1080.0;

pub struct SkiaRenderer {
    render_ctx: VulkanRenderContext,
    renderer: Option<VulkanRenderer>,
    assets: AssetManager,
    audio_player: AudioPlayer,
    painter: Painter,
    pub font_collection: FontCollection,

    shaders: HashMap<String, RuntimeEffect>,
    screens: Vec<Box<dyn Screen>>,
    start_time: Instant,
    ctx: Ctx,

    ui_ctx: UiContext,
    physical_cursor_pos: (f32, f32),
    scale_factor: f64,

    gc_timer: Instant,
    last_frame: Instant,
}

impl SkiaRenderer {
    pub fn new(manager: Arc<ScriptManager>) -> Self {
        let sys_cfg: SystemConfig = lumina_shared::config::get("system");
        let assets = AssetManager::new(&sys_cfg.assets_path);

        let mut font_collection = FontCollection::new();
        let mut font_provider = TypefaceFontProvider::new();
        assets.register_fonts_to(&mut font_provider);
        font_collection.set_asset_font_manager(Some(font_provider.into()));
        font_collection.set_dynamic_font_manager(FontMgr::default());

        let mut shaders = HashMap::new();
        let shader_dir = Path::new(&sys_cfg.script_path).join("system/core");
        let trans_shader_path = shader_dir.join("transition.sksl");

        if trans_shader_path.exists() {
            match fs::read_to_string(&trans_shader_path) {
                Ok(code) => {
                    match RuntimeEffect::make_for_shader(&code, None) {
                        Ok(effect) => {
                            log::info!("Loaded shader: transition");
                            shaders.insert("transition".to_string(), effect);
                        },
                        Err(err) => {
                            log::error!("Failed to compile shader {:?}: {}", trans_shader_path, err);
                        }
                    }
                },
                Err(e) => log::error!("Failed to read shader file: {}", e),
            }
        } else {
            log::warn!("Shader file not found: {:?}", trans_shader_path);
        }

        let mut ctx = Ctx::default();

        let driver = lumina_core::renderer::driver::ExecutorHandle::new(&mut ctx, manager.clone());
        let initial_screen: Box<dyn Screen> = Box::new(GameScreen::new(driver));

        Self {
            render_ctx: VulkanRenderContext::default(),
            renderer: None,
            assets,
            audio_player: AudioPlayer::new(),
            painter: Painter::new(),
            font_collection,

            shaders,
            screens: vec![initial_screen],
            start_time: Instant::now(),
            ctx,

            ui_ctx: UiContext::new(),
            physical_cursor_pos: (0.0, 0.0),
            scale_factor: 1.0,

            gc_timer: Instant::now(),
            last_frame: Instant::now(),
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self).unwrap();
    }

    fn request_redraw(&self) {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.window.request_redraw();
        }
    }


    fn to_logical(physical_x: f32, physical_y: f32, scale: f32, off_x: f32, off_y: f32) -> (f32, f32) {
        if scale == 0.0 { return (0.0, 0.0); }
        (
            (physical_x - off_x) / scale,
            (physical_y - off_y) / scale
        )
    }
}

impl ApplicationHandler for SkiaRenderer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let cfg: WindowConfig = lumina_shared::config::get("window");
        let window_attributes = Window::default_attributes()
            .with_title(&cfg.title)
            .with_inner_size(PhysicalSize::new(cfg.width, cfg.height))
            .with_resizable(cfg.resizable);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.scale_factor = window.scale_factor();
        self.renderer = Some(self.render_ctx.renderer_for_window(event_loop, window.clone(), cfg.vsync));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(_) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.invalidate_swapchain();
                    self.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = scale_factor;
                self.request_redraw();
            },
            // 1. 鼠标移动：记录物理坐标
            WindowEvent::CursorMoved { position, .. } => {
                // 注意：这里必须是 f32
                self.physical_cursor_pos = (position.x as f32, position.y as f32);
                self.request_redraw();
            },

            // 2. 点击：记录状态
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let pressed = state == ElementState::Pressed;
                self.ui_ctx.mouse_pressed = pressed && !self.ui_ctx.mouse_held;
                self.ui_ctx.mouse_held = pressed;
                self.request_redraw();
            },

            WindowEvent::RedrawRequested => {
                self.assets.update();
                self.audio_player.update(&mut self.assets);

                let now = Instant::now();
                let dt = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;

                let mut transition = ScreenTransition::None;

                if let Some(screen) = self.screens.last_mut() {
                    transition = screen.update(
                        dt,
                        &mut self.ctx,
                        event_loop,
                        &mut self.assets,
                        &mut self.audio_player
                    );
                }

                match transition {
                    ScreenTransition::Quit => event_loop.exit(),
                    ScreenTransition::None => {},
                }

                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.prepare_swapchain();

                    // 准备引用，供闭包使用
                    let screens_ref = &mut self.screens;
                    let ctx_ref = &mut self.ctx;
                    let ui_ctx_ref = &mut self.ui_ctx;
                    let painter_ref = &mut self.painter;
                    let assets_ref = &mut self.assets;
                    let fonts_ref = &self.font_collection;
                    let shaders_ref = &self.shaders;

                    let time = self.start_time.elapsed().as_secs_f32();

                    let (mx, my) = self.physical_cursor_pos;
                    let phy_win_size = renderer.window.inner_size();

                    renderer.draw_and_present(|canvas, size| {
                        // A. 布局计算 (含 DPI 修正)
                        let win_w = size.width;
                        let win_h = size.height;

                        let phy_w = phy_win_size.width as f32;
                        let content_scale = if phy_w > 0.0 { win_w / phy_w } else { 1.0 };
                        let adj_mx = mx * content_scale;
                        let adj_my = my * content_scale;

                        let scale_x = win_w / DESIGN_WIDTH;
                        let scale_y = win_h / DESIGN_HEIGHT;
                        let scale = scale_x.min(scale_y);
                        let off_x = (win_w - DESIGN_WIDTH * scale) / 2.0;
                        let off_y = (win_h - DESIGN_HEIGHT * scale) / 2.0;

                        // B. 更新 UI 鼠标状态
                        let (lx, ly) = SkiaRenderer::to_logical(adj_mx, adj_my, scale, off_x, off_y);
                        ui_ctx_ref.update(lx, ly, ui_ctx_ref.mouse_pressed, ui_ctx_ref.mouse_held);

                        // C. 设置画布
                        canvas.save();
                        canvas.clear(skia_safe::Color::BLACK); // 默认清黑屏
                        canvas.translate(skia_safe::Vector::new(off_x, off_y));
                        canvas.scale((scale, scale));
                        canvas.clip_rect(skia_safe::Rect::new(0.0, 0.0, DESIGN_WIDTH, DESIGN_HEIGHT), None, None);

                        // D. 委托给栈顶 Screen 绘制
                        if let Some(screen) = screens_ref.last_mut() {
                            let mut ui = UiDrawer::new(canvas, ui_ctx_ref, fonts_ref, assets_ref, time, shaders_ref);
                            let design_rect = Rect::new(0.0, 0.0, DESIGN_WIDTH, DESIGN_HEIGHT);

                            screen.draw(
                                &mut ui,
                                painter_ref,
                                design_rect,
                                ctx_ref
                            );
                        }

                        canvas.restore();
                    });

                    self.ui_ctx.mouse_pressed = false;

                    if self.gc_timer.elapsed().as_secs() >= 30 {
                        self.assets.gc(Duration::from_secs(60));
                        self.gc_timer = Instant::now();
                    }
                    renderer.window.request_redraw();
                }
            },
            _ => {}
        }
    }
}