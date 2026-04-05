use std::collections::HashMap;
use std::sync::Arc;

use super::{Screen, ScreenTransition};
use super::ui_interp;
use crate::ui::UiDrawer;
use crate::core::{AssetManager, Painter, AudioPlayer, Typewriter};
use crate::core::SceneAnimator;
use lumina_core::{Ctx, OutputEvent};
use lumina_core::event::InputEvent;
use lumina_core::renderer::driver::ExecutorHandle;
use lumina_core::typewriter_bridge::TypewriterBridge;
use lumina_ui::{Rect, UiRenderer};
use viviscript_core::ast::UiStmt;
use winit::event_loop::ActiveEventLoop;

struct ActiveScreen {
    id: String,
    overlay: bool,
}

pub struct GameScreen {
    driver: ExecutorHandle,
    animator: SceneAnimator,

    /// vivi 定义的 screen 注册表
    screen_registry: HashMap<String, Arc<[UiStmt]>>,
    /// 当前活跃的 screens（按入栈顺序渲染）
    active_screens: Vec<ActiveScreen>,
    /// 是否被非 overlay screen 阻塞脚本执行
    blocked_by_screen: bool,
    /// 按钮点击产生的 action 字符串，在 update 阶段处理
    pending_actions: Vec<String>,

    /// 打字机动画（由 Lua 通过 bridge 控制）
    typewriter: Typewriter,
    tw_bridge: TypewriterBridge,

    /// 脚本执行完毕（call_stack 耗尽）但仍有 screen 阻塞时置 true，
    /// 防止 OutputEvent::End 立即关闭窗口
    script_ended: bool,
}

impl GameScreen {
    /// 创建游戏主屏，持有执行器句柄和场景动画器。
    pub fn new(driver: ExecutorHandle) -> Self {
        let tw_bridge = driver.typewriter_bridge();
        let mut animator = SceneAnimator::new();
        animator.resize(1920.0, 1080.0);

        Self {
            driver,
            animator,
            screen_registry: HashMap::new(),
            active_screens: Vec::new(),
            blocked_by_screen: false,
            pending_actions: Vec::new(),
            typewriter: Typewriter::new(),
            tw_bridge,
            script_ended: false,
        }
    }

    /// 消费 ctx 事件队列：将精灵/背景/音频/UI 屏等事件分发给动画器、音频播放器和内部状态。
    fn process_output_events(
        &mut self,
        ctx: &mut Ctx,
        el: &ActiveEventLoop,
        assets: &mut AssetManager,
        audio: &mut AudioPlayer,
    ) {
        let events: Vec<_> = ctx.drain().into_iter().collect();

        let get_sprite_info = |target: &str| -> (Option<String>, Option<Vec<String>>) {
            if let Some(layer) = ctx.layer_record.layer.get("master") {
                if let Some(s) = layer.iter().find(|s| s.target == target) {
                    return (s.position.clone(), Some(s.attrs.clone()));
                }
            }
            (None, None)
        };

        for event in events {
            match event {
                // ── 音频 ───────────────────────────────────────────────
                OutputEvent::PlayAudio { channel, path, fade_in, volume, looping } => {
                    audio.play(assets, &channel, &path, volume, fade_in, looping);
                }
                OutputEvent::StopAudio { channel, fade_out } => {
                    audio.stop(&channel, fade_out);
                }
                OutputEvent::SetVolume { channel, value } => {
                    audio.set_channel_volume(&channel, value);
                }

                // ── 精灵/场景 ──────────────────────────────────────────
                OutputEvent::NewSprite { target, texture, pos_str, transition, attrs, defer_visual } => {
                    self.animator.handle_new_sprite(target, texture, pos_str.as_deref(), transition, attrs, defer_visual);
                }
                OutputEvent::UpdateSprite { target, transition } => {
                    let (pos_str, attrs) = get_sprite_info(&target);
                    self.animator.handle_update_sprite(target, transition, pos_str.as_deref(), attrs.unwrap_or_default());
                }
                OutputEvent::HideSprite { target, transition } => {
                    self.animator.handle_hide_sprite(target, transition);
                }
                OutputEvent::NewScene { transition } => {
                    let mut bg_name = None;
                    if let Some(layer) = ctx.layer_record.layer.get("master") {
                        if let Some(bg) = layer.first() {
                            let mut full_name = bg.target.clone();
                            if !bg.attrs.is_empty() {
                                full_name.push('_');
                                full_name.push_str(&bg.attrs.join("_"));
                            }
                            bg_name = Some(full_name);
                        }
                    }
                    self.animator.handle_new_scene(bg_name, transition);
                }
                OutputEvent::Preload { images, audios } => {
                    for img_id in images { assets.get_image(&img_id); }
                    for audio_id in audios { assets.get_static_audio(&audio_id); }
                }
                OutputEvent::ModifyVisual { target, props, duration, easing } => {
                    self.animator.handle_modify_visual(target, props, duration, easing);
                }
                OutputEvent::RegisterLayout { name, config } => {
                    self.animator.handle_register_layout(name, config);
                }
                OutputEvent::RegisterTransition { name, config } => {
                    self.animator.handle_register_transition(name, config);
                }

                // ── UI Screen ──────────────────────────────────────────
                OutputEvent::RegisterScreen { id, def } => {
                    self.screen_registry.insert(id, def);
                }
                OutputEvent::ShowScreen { id, overlay } => {
                    // 同一个 id 不重复添加
                    if !self.active_screens.iter().any(|s| s.id == id) {
                        if !overlay {
                            self.blocked_by_screen = true;
                        }
                        self.active_screens.push(ActiveScreen { id, overlay });
                    }
                }
                OutputEvent::HideScreen { id } => {
                    self.active_screens.retain(|s| s.id != id);
                    self.refresh_block_state();
                }

                OutputEvent::End => {
                    if self.blocked_by_screen {
                        self.script_ended = true;
                    } else {
                        el.exit();
                    }
                }

                // ShowDialogue / ShowNarration / ShowChoice 由 Lua 回调处理，
                // Skia 端不再硬编码渲染
                _ => {}
            }
        }
    }

    /// 重新计算 `blocked_by_screen`：只要有非 overlay 的活跃屏就阻塞脚本。
    fn refresh_block_state(&mut self) {
        self.blocked_by_screen = self.active_screens.iter().any(|s| !s.overlay);
    }

    /// 处理本帧累积的 UI 按钮动作（continue / quit / jump / choice）。
    fn process_pending_actions(&mut self, ctx: &mut Ctx) {
        let actions: Vec<String> = self.pending_actions.drain(..).collect();
        for action in actions {
            let trimmed = action.trim();
            if trimmed == "continue" {
                // 关闭所有阻塞性 screen，恢复脚本并推进
                self.active_screens.retain(|s| s.overlay);
                self.refresh_block_state();
                self.driver.feed(ctx, InputEvent::Continue);
            } else if trimmed == "quit" {
                // 退出由 update() 的返回值处理，这里标记一下
                self.pending_actions.push("__quit__".to_string());
            } else if let Some(label) = trimmed.strip_prefix("jump ") {
                self.active_screens.retain(|s| s.overlay);
                self.refresh_block_state();
                self.script_ended = false;
                self.driver.feed(ctx, InputEvent::Jump(label.trim().to_string()));
            } else if let Some(id) = trimmed.strip_prefix("show ") {
                let id = id.trim().to_string();
                if !self.active_screens.iter().any(|s| s.id == id) {
                    self.active_screens.push(ActiveScreen { id, overlay: false });
                    self.blocked_by_screen = true;
                }
            } else if trimmed == "hide screen" {
                // 关闭最顶层非 overlay screen
                if let Some(pos) = self.active_screens.iter().rposition(|s| !s.overlay) {
                    self.active_screens.remove(pos);
                }
                self.refresh_block_state();
            } else if let Some(rest) = trimmed.strip_prefix("choice ") {
                if let Ok(idx) = rest.trim().parse::<usize>() {
                    self.active_screens.retain(|s| s.overlay);
                    self.refresh_block_state();
                    self.driver.feed(ctx, InputEvent::ChoiceMade { index: idx });
                }
            } else {
                // Fallback：当作 Lua 函数名调用
                log::debug!("UI action fallback to Lua: {}", trimmed);
            }
        }
    }
}

impl Screen for GameScreen {
    fn update(
        &mut self,
        dt: f32,
        ctx: &mut Ctx,
        el: &ActiveEventLoop,
        assets: &mut AssetManager,
        audio: &mut AudioPlayer,
    ) -> ScreenTransition {
        // 1. 始终排空 Lua 命令（即使脚本被 screen 阻塞，Lua tween 动画仍需处理）
        self.driver.drain_commands(ctx);

        // 2. 驱动 VM（若未被 screen 阻塞，且脚本尚未结束）
        if !self.blocked_by_screen && !self.script_ended {
            for _ in 0..100 {
                if self.driver.step(ctx) { break; }
            }
        }

        // 3. 处理输出事件
        self.process_output_events(ctx, el, assets, audio);

        // 4. 处理按钮 action
        let wants_quit = self.pending_actions.contains(&"__quit__".to_string());
        self.process_pending_actions(ctx);
        if wants_quit {
            return ScreenTransition::Quit;
        }

        // 5. 同步打字机状态
        if let Some(text) = self.tw_bridge.take_pending_set() {
            self.typewriter.set_text("", &text, "", " ▼");
        }
        if self.tw_bridge.take_skip() {
            self.typewriter.skip();
        }
        self.typewriter.update(dt);
        self.tw_bridge.update_display(
            self.typewriter.display_text.clone(),
            !self.typewriter.is_active(),
        );

        // 6. 更新动画和 Lua tick
        self.animator.update(dt);
        self.driver.tick(dt);

        ScreenTransition::None
    }

    fn draw(&mut self, ui: &mut UiDrawer, painter: &mut Painter, rect: Rect, ctx: &mut Ctx) {
        // 1. 渲染精灵/背景
        painter.paint(ui, &self.animator, (rect.w, rect.h));

        // 2. 渲染 active vivi screens
        // 需要临时借用 screen_registry 和 active_screens
        let screen_ids: Vec<(String, bool)> = self.active_screens
            .iter()
            .map(|s| (s.id.clone(), s.overlay))
            .collect();

        for (id, _overlay) in &screen_ids {
            if let Some(def) = self.screen_registry.get(id.as_str()) {
                let def = def.clone();
                ui_interp::render(
                    ui,
                    self.driver.lua(),
                    &def,
                    rect,
                    &mut self.pending_actions,
                );
            }
        }

        // 3. 全局点击继续（仅无阻塞 screen 时）
        if !self.blocked_by_screen && ui.interact(rect).is_clicked() {
            if self.animator.is_busy() {
                self.animator.finish_all_animations();
            } else if self.typewriter.is_active() {
                self.typewriter.skip();
            } else {
                self.driver.feed(ctx, InputEvent::Continue);
            }
        }
    }
}
