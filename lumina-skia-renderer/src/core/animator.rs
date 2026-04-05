use std::collections::HashMap;
use lumina_core::event::{LayoutConfig, TransitionConfig};

#[derive(Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// 构造二维向量。
    pub fn new(x: f32, y: f32) -> Self { Self { x, y } }
}

#[derive(Clone, Debug)]
pub struct RenderSprite {
    pub target: String,
    pub texture: String,
    pub attrs: Vec<String>,

    pub old_texture: Option<String>,
    pub rule_texture: Option<String>,
    pub trans_progress: f32,
    pub trans_vague: f32,
    pub in_transition: bool,

    pub pos: Vec2,
    pub offset: Vec2,
    pub scale: f32,
    pub alpha: f32,
    pub rotation: f32,
    pub anchor: Vec2,
    pub z_index: i32,

    pub pending_data: bool,
}

impl RenderSprite {
    /// 创建渲染精灵，初始 alpha=1、scale=1，位置待后续设置。
    pub fn new(target: String, texture: String, attrs: Vec<String>) -> Self {
        Self {
            target,
            texture,
            attrs,
            old_texture: None,
            rule_texture: None,
            trans_progress: 1.0,
            trans_vague: 0.1,
            in_transition: false,
            pos: Vec2::new(0.0, 0.0),
            offset: Vec2::new(0.0, 0.0),
            scale: 1.0,
            alpha: 1.0,
            rotation: 0.0,
            anchor: Vec2::new(0.5, 1.0),
            z_index: 0,
            pending_data: false,
        }
    }
    /// 拼接贴图名和属性后缀，如 `"ch1_happy"`。
    pub fn full_asset_name(&self) -> String {
        if self.attrs.is_empty() {
            return self.texture.clone();
        }
        let mut name = self.texture.clone();
        for attr in &self.attrs {
            name.push('_');
            name.push_str(attr);
        }
        name
    }

    /// 按名称设置精灵属性（x/y/alpha/scale/offset_x 等），同时清除 `pending_data` 标记。
    pub fn set_prop(&mut self, key: &str, val: f32) {
        self.pending_data = false;
        match key {
            "x" => self.pos.x = val,
            "y" => self.pos.y = val,
            "offset_x" | "ox" => self.offset.x = val,
            "offset_y" | "oy" => self.offset.y = val,
            "scale" | "scale_x" | "scale_y" => self.scale = val, // 确保这里覆盖了所有 Lua 可能发的 key
            "alpha" | "opacity" => self.alpha = val.clamp(0.0, 1.0),
            "rotation" | "angle" => self.rotation = val,
            "trans_progress" => self.trans_progress = val.clamp(0.0, 1.0),
            "trans_vague" => self.trans_vague = val,
            _ => {
                log::warn!("RenderSprite: Unknown prop '{}'", key);
            }
        }
    }

    /// 按名称读取精灵属性当前值，未知属性返回 `0.0`。
    pub fn get_prop(&self, key: &str) -> f32 {
        match key {
            "x" => self.pos.x,
            "y" => self.pos.y,
            "offset_x" | "ox" => self.offset.x,
            "offset_y" | "oy" => self.offset.y,
            "alpha" | "opacity" => self.alpha,
            "scale" => self.scale,
            "rotation" | "angle" => self.rotation,
            _ => 0.0,
        }
    }
}

struct GenericTweener {
    target: String,
    duration: f32,
    elapsed: f32,
    // 存储 (属性名, (起始值, 目标值))
    props: HashMap<String, (f32, f32)>,
    easing: String,
}

pub struct SceneAnimator {
    pub sprites: HashMap<String, RenderSprite>,
    generic_tweens: Vec<GenericTweener>,
    screen_size: (f32, f32),

    layouts: HashMap<String, LayoutConfig>,
    trans_registry: HashMap<String, TransitionConfig>,
}

impl SceneAnimator {
    /// 创建场景动画器，预置 left/center/right 三个内置布局。
    pub fn new() -> Self {
        let mut layouts = HashMap::new();
        layouts.insert("center".into(), LayoutConfig { x: 0.5, y: 1.0, anchor_x: 0.5, anchor_y: 1.0 });
        layouts.insert("left".into(), LayoutConfig { x: 0.2, y: 1.0, anchor_x: 0.5, anchor_y: 1.0 });
        layouts.insert("right".into(), LayoutConfig { x: 0.8, y: 1.0, anchor_x: 0.5, anchor_y: 1.0 });

        Self {
            sprites: HashMap::new(),
            generic_tweens: Vec::new(),
            screen_size: (1920.0, 1080.0),
            layouts,
            trans_registry: HashMap::new(),
        }
    }
    /// 注册或覆盖一个命名布局（屏幕位置/锚点配置）。
    pub fn handle_register_layout(&mut self, name: String, config: LayoutConfig) {
        self.layouts.insert(name, config);
    }
    /// 注册或覆盖一个静态过渡效果配置（由 Lua `lumina.register_transition` 触发）。
    pub fn handle_register_transition(&mut self, name: String, config: TransitionConfig) {
        self.trans_registry.insert(name, config);
    }

    /// 更新逻辑分辨率，影响布局计算中的绝对坐标转换。
    pub fn resize(&mut self, w: f32, h: f32) {
        self.screen_size = (w, h);
    }

    /// 若有仍在播放的补间动画则返回 `true`。
    pub fn is_busy(&self) -> bool {
        !self.generic_tweens.is_empty()
    }

    /// 立即跳过所有进行中的补间动画到终态（用于快进/跳过）。
    pub fn finish_all_animations(&mut self) {
        if self.generic_tweens.is_empty() { return; }

        // log::debug!("Skipping {} animations", self.generic_tweens.len());

        for tween in &mut self.generic_tweens {
            tween.elapsed = tween.duration;
        }

        self.update(0.0);

        for sprite in self.sprites.values_mut() {
            if sprite.in_transition {
                sprite.in_transition = false;
                sprite.trans_progress = 1.0;
                sprite.old_texture = None;
            }
        }
    }

    /// 推进所有补间动画一帧，清理已完成动画及 alpha 为 0 的精灵。
    pub fn update(&mut self, dt: f32) {
        let mut finished = Vec::new();

        for (i, tween) in self.generic_tweens.iter_mut().enumerate() {
            tween.elapsed += dt;
            let t = if tween.duration <= 0.0001 {
                1.0
            } else {
                (tween.elapsed / tween.duration).clamp(0.0, 1.0)
            };

            let progress = match tween.easing.as_str() {
                "linear" => t,
                "ease_out" => t * (2.0 - t),
                "ease_in" => t * t,
                "ease_in_out" => if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t },
                _ => t,
            };

            if let Some(sprite) = self.sprites.get_mut(&tween.target) {
                for (key, (start_val, end_val)) in &tween.props {
                    let current_val = start_val + (end_val - start_val) * progress;
                    sprite.set_prop(key, current_val);
                }
            }

            if t >= 1.0 {
                finished.push(i);
            }
        }

        for i in finished.iter().rev() {
            self.generic_tweens.remove(*i);
        }

        self.sprites.retain(|target, sprite| {
            let is_visible = sprite.alpha > 0.001;
            let has_active_tween = self.generic_tweens.iter().any(|t| t.target == *target);
            is_visible || has_active_tween
        });
    }

    /// 修改精灵属性：`duration=0` 立即生效，否则创建补间动画（由 Lua `lumina.transform` 触发）。
    pub fn handle_modify_visual(
        &mut self,
        target: String,
        props: HashMap<String, f32>,
        duration: f32,
        easing: String
    ) {
        if let Some(sprite) = self.sprites.get_mut(&target) {
            self.generic_tweens.retain(|t| t.target != target);
            if duration <= 0.001 {
                for (k, v) in props {
                    sprite.set_prop(&k, v);
                }
            } else {
                let mut tween_props = HashMap::new();
                for (k, target_val) in props {
                    tween_props.insert(k.clone(), (sprite.get_prop(&k), target_val));
                }
                self.generic_tweens.push(GenericTweener {
                    target, duration, elapsed: 0.0, props: tween_props, easing
                });
            }
        }
    }

    /// 添加新精灵到场景。`defer_visual=true` 时设置 `pending_data` 标记，精灵在
    /// 收到首次 `modify_visual` 前不渲染（用于 Lua 动态入场效果）。
    pub fn handle_new_sprite(&mut self, target: String, texture: String, pos_str: Option<&str>, trans: Option<String>, attrs: Vec<String>, defer_visual: bool) {
        let mut sprite = RenderSprite::new(target.clone(), texture, attrs);

        let layout_key = pos_str.unwrap_or("center");
        let layout = self.layouts.get(layout_key).cloned().unwrap_or(LayoutConfig {
            x: 0.5, y: 1.0, anchor_x: 0.5, anchor_y: 1.0
        });

        if defer_visual {
            sprite.pending_data = true;
        }

        let (w, h) = self.screen_size;
        sprite.pos = Vec2::new(layout.x * w, layout.y * h);
        sprite.anchor = Vec2::new(layout.anchor_x, layout.anchor_y);

        if let Some(trans_name) = trans {
            if let Some(cfg) = self.trans_registry.get(&trans_name).cloned() {
                // 有静态配置
                let mut tween_props = HashMap::new();
                for (k, (from_opt, to_val)) in cfg.props {
                    // 如果定义了 from，立即应用初始值
                    if let Some(from_val) = from_opt {
                        sprite.set_prop(&k, from_val);
                        tween_props.insert(k, (from_val, to_val));
                    } else {
                        // 如果没定义 from，取当前值 (对于 NewSprite 来说通常是默认值)
                        tween_props.insert(k.clone(), (sprite.get_prop(&k), to_val));
                    }
                }

                self.generic_tweens.push(GenericTweener {
                    target: target.clone(),
                    duration: cfg.duration,
                    elapsed: 0.0,
                    props: tween_props,
                    easing: cfg.easing,
                });
            }
        }

        self.sprites.insert(target, sprite);
    }

    /// 更新已有精灵的位置/属性，可附带过渡动画；过渡未找到时直接切换。
    pub fn handle_update_sprite(&mut self, target: String, trans: String, new_pos: Option<&str>, new_attrs: Vec<String>) {
        if let Some(sprite) = self.sprites.get_mut(&target) {
            let target_pos_vec = if let Some(pos_key) = new_pos {
                let layout = self.layouts.get(pos_key).cloned().unwrap_or(LayoutConfig {
                    x: 0.5, y: 1.0, anchor_x: 0.5, anchor_y: 1.0
                });
                let (w, h) = self.screen_size;
                Some(Vec2::new(layout.x * w, layout.y * h))
            } else {
                None
            };

            let mut visual_changed = false;
            let current_full_name = sprite.full_asset_name();

            if !new_attrs.is_empty() && new_attrs != sprite.attrs {
                visual_changed = true;
            }

            let mut applied_transition = false;

            if !trans.is_empty() {
                if let Some(cfg) = self.trans_registry.get(&trans).cloned() {
                    let mut tween_props = HashMap::new();
                    if visual_changed {
                        sprite.old_texture = Some(current_full_name);
                        sprite.rule_texture = cfg.mask_img.clone();
                        sprite.trans_vague = cfg.vague.unwrap_or(0.1);
                        sprite.in_transition = true;
                        sprite.trans_progress = 0.0;

                        sprite.attrs = new_attrs.clone();
                        tween_props.insert("trans_progress".to_string(), (0.0, 1.0));
                    } else {
                        if !new_attrs.is_empty() { sprite.attrs = new_attrs.clone(); }
                    }

                    if let Some(tp) = target_pos_vec {
                        // 除非转场配置里显式覆盖了 x/y (比如震动效果)，否则自动补间过去
                        if !cfg.props.contains_key("x") {
                            tween_props.insert("x".to_string(), (sprite.pos.x, tp.x));
                        }
                        if !cfg.props.contains_key("y") {
                            tween_props.insert("y".to_string(), (sprite.pos.y, tp.y));
                        }
                    }

                    for (k, (from_opt, to_val)) in cfg.props {
                        let start = from_opt.unwrap_or(sprite.get_prop(&k));
                        tween_props.insert(k, (start, to_val));
                    }

                    self.generic_tweens.retain(|t| t.target != target);
                    self.generic_tweens.push(GenericTweener {
                        target: target.clone(),
                        duration: cfg.duration,
                        elapsed: 0.0,
                        props: tween_props,
                        easing: cfg.easing,
                    });
                    applied_transition = true;
                } else {
                    log::warn!("Transition '{}' not found, falling back to instant update.", trans);
                }
            }

            if !applied_transition {
                if !new_attrs.is_empty() {
                    sprite.attrs = new_attrs;
                }

                if let Some(tp) = target_pos_vec {
                    sprite.pos = tp;
                }

                sprite.in_transition = false;
                sprite.trans_progress = 1.0;
                sprite.old_texture = None;
            }
        }
    }

    /// 隐藏精灵：有过渡则播放退场动画（alpha→0），动画结束后自动从 sprites 中移除；无过渡则立即删除。
    pub fn handle_hide_sprite(&mut self, target: String, trans: Option<String>) {
        if let Some(t_name) = trans {
            if let Some(cfg) = self.trans_registry.get(&t_name).cloned() {
                let mut tween_props = HashMap::new();
                for (k, (from_opt, to_val)) in cfg.props {
                    let start = from_opt.unwrap_or_else(|| self.sprites.get(&target).map(|s| s.get_prop(&k)).unwrap_or(0.0));
                    tween_props.insert(k, (start, to_val));
                }
                self.generic_tweens.push(GenericTweener {
                    target: target.clone(), duration: cfg.duration, elapsed: 0.0, props: tween_props, easing: cfg.easing
                });
                return;
            }
        }
        self.sprites.remove(&target);
        self.generic_tweens.retain(|t| t.target != target);
    }

    /// 切换背景场景：清除非 bg 精灵，若背景已存在则执行纹理过渡，否则直接创建。
    pub fn handle_new_scene(&mut self, bg_name: Option<String>, trans: String) {
        self.sprites.retain(|key, _| key == "bg");
        self.generic_tweens.retain(|t| t.target == "bg");

        let new_bg_tex = bg_name.unwrap_or_default();

        if new_bg_tex.is_empty() {
            self.sprites.remove("bg");
            return;
        }

        if self.sprites.contains_key("bg") {
            if let Some(cfg) = self.trans_registry.get(&trans).cloned() {
                self.strat_texture_transition("bg".to_string(), new_bg_tex, cfg);
            } else {
                if let Some(s) = self.sprites.get_mut("bg") {
                    s.texture = new_bg_tex;
                    s.in_transition = false;
                    s.trans_progress = 1.0;
                }
            }
        } else {
            let mut bg_sprite = RenderSprite::new("bg".to_string(), new_bg_tex, vec![]);
            bg_sprite.z_index = -100;
            bg_sprite.anchor = Vec2::new(0.0, 0.0);
            self.sprites.insert("bg".to_string(), bg_sprite);
        }
    }

    fn strat_texture_transition(&mut self, target: String, new_tex: String, trans_cfg: TransitionConfig) {
        if let Some(sprite) = self.sprites.get_mut(&target) {
            if sprite.texture == new_tex { return; }
            sprite.old_texture = Some(sprite.texture.clone());
            sprite.rule_texture = trans_cfg.mask_img.clone();

            sprite.texture = new_tex;
            sprite.in_transition = true;
            sprite.trans_progress = 0.0; // 进度归零
            sprite.trans_vague = trans_cfg.vague.unwrap_or(0.1);

            let mut props = std::collections::HashMap::new();
            props.insert("trans_progress".to_string(), (0.0, 1.0));

            self.generic_tweens.retain(|t| t.target != target);
            self.generic_tweens.push(GenericTweener {
                target,
                duration: trans_cfg.duration,
                elapsed: 0.0,
                props,
                easing: trans_cfg.easing,
            });
        }
    }

}
