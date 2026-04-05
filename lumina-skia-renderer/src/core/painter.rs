use crate::core::animator::{RenderSprite, SceneAnimator};
use lumina_ui::{Color, Rect, ShaderSpec, Transform, UiRenderer};
use std::borrow::Cow;
use std::path::Path;

pub struct Painter {
}

impl Painter {
    /// 创建画笔实例（无状态，绘制参数按帧传入）。
    pub fn new() -> Self {
        Self {}
    }

    fn extract_key(path_str: &str) -> Cow<'_, str> {
        let path = Path::new(path_str);
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            Cow::Owned(stem.to_string())
        } else {
            Cow::Borrowed(path_str)
        }
    }

    /// 将动画器中所有精灵按 z-index 排序后绘制到画布：
    /// 纹理过渡中的精灵使用 transition shader，其余直接绘制并应用 alpha。
    pub fn paint(
        &mut self,
        ui: &mut impl UiRenderer,
        animator: &SceneAnimator,
        window_size: (f32, f32),
    ) {
        let (win_w, win_h) = window_size;

        let mut render_list: Vec<&RenderSprite> = animator.sprites.values().collect();
        render_list.sort_by(|a, b| a.z_index.cmp(&b.z_index));

        for sprite in render_list {
            if sprite.pending_data { continue; }
            let full_name = sprite.full_asset_name();
            let (raw_w, raw_h) = ui.measure_image(&full_name).unwrap_or((100.0, 100.0));
            let is_bg = sprite.z_index < 0;
            let draw_rect = if is_bg {
                // 背景：强制铺满窗口
                Rect::new(0.0, 0.0, win_w, win_h)
            } else {
                // 立绘：根据锚点计算相对偏移
                let offset_x = -raw_w * sprite.anchor.x;
                let offset_y = -raw_h * sprite.anchor.y;
                Rect::new(offset_x, offset_y, raw_w, raw_h)
            };

            let mut t = Transform::default();
            if !is_bg {
                t.x = sprite.pos.x + sprite.offset.x;
                t.y = sprite.pos.y + sprite.offset.y;
                t.rotation = sprite.rotation;
                t.scale_x = sprite.scale;
                t.scale_y = sprite.scale;
            }

            let mut drawn = false;
            if sprite.in_transition && sprite.trans_progress < 1.0 {
                let name_old = sprite.old_texture.clone().unwrap_or_default();

                let name_rule = sprite.rule_texture.clone().unwrap_or_default();
                let key_rule = Self::extract_key(&name_rule);
                let use_rule = if !name_rule.is_empty() { 1.0 } else { 0.0 };
                let uniforms = [
                    sprite.trans_progress, // u_progress
                    sprite.trans_vague,    // u_vague
                    use_rule               // u_use_rule
                ];

                let images = [
                    name_old.as_ref(), // texture 0 (Old)
                    full_name.as_ref(), // texture 1 (New)
                    key_rule.as_ref() // texture 2 (Rule/Mask)
                ];

                let spec = ShaderSpec {
                    shader_id: "transition",
                    uniforms: &uniforms,
                    images: &images,
                };

                if is_bg {
                    ui.draw_shader(draw_rect, spec);
                } else {
                    ui.with_transform(t, &mut |ui| {
                        ui.draw_shader(draw_rect, spec);
                    });
                }
                drawn = true;
            }
            if !drawn {
                let alpha_byte = (sprite.alpha * 255.0) as u8;
                let tint = Color::rgba(255, 255, 255, alpha_byte);

                if is_bg {
                    ui.draw_image(&full_name, draw_rect, tint);
                } else {
                    ui.with_transform(t, &mut |ui| {
                        ui.draw_image(&full_name, draw_rect, tint);
                    });
                }
            }
        }
    }
}