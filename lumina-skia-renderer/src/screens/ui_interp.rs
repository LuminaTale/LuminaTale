//! UiStmt 解释渲染器
//!
//! 将 viviscript 解析出的 `UiStmt` 树通过 IMGUI + 每帧 Layout Pass 渲染到屏幕上。

use mlua::Lua;
use viviscript_core::ast::{ContainerKind, UiStmt, WidgetKind};
use lumina_ui::{Alignment, Color, Rect, UiRenderer};
use lumina_ui::widgets::{Button, Label, Panel};

// ──────────────────────────────────────────────────────────────
// 公共入口
// ──────────────────────────────────────────────────────────────

/// 渲染一组 UiStmt，写入点击产生的 action 字符串
pub fn render(
    ui: &mut impl UiRenderer,
    lua: &Lua,
    stmts: &[UiStmt],
    parent_rect: Rect,
    actions: &mut Vec<String>,
) {
    for stmt in stmts {
        render_stmt(ui, lua, stmt, parent_rect, actions);
    }
}

// ──────────────────────────────────────────────────────────────
// 内部实现
// ──────────────────────────────────────────────────────────────

fn render_stmt(
    ui: &mut impl UiRenderer,
    lua: &Lua,
    stmt: &UiStmt,
    parent_rect: Rect,
    actions: &mut Vec<String>,
) {
    match stmt {
        UiStmt::Container { kind, props, children, .. } => {
            let rect = apply_frame_props(parent_rect, props);
            render_container(ui, lua, *kind, props, children, rect, actions);
        }
        UiStmt::Widget { kind, value, props, .. } => {
            let rect = apply_frame_props(parent_rect, props);
            render_widget(ui, lua, *kind, value.as_deref(), props, rect, actions);
        }
    }
}

// ── 容器渲染 ────────────────────────────────────────────────────

fn render_container(
    ui: &mut impl UiRenderer,
    lua: &Lua,
    kind: ContainerKind,
    props: &[viviscript_core::ast::UiProp],
    children: &[UiStmt],
    rect: Rect,
    actions: &mut Vec<String>,
) {
    // 绘制背景（如果有 bg prop）
    if let Some(bg_color) = get_color_prop(props, "bg") {
        use lumina_ui::{Style, Background};
        let mut style = Style::default();
        style.background = Background::Solid(bg_color);
        if let Some(r) = get_f32_prop(props, "border_radius") {
            style.border.radius = r;
        }
        ui.draw_style(rect, &style);
    }

    match kind {
        ContainerKind::ZBox => {
            // 所有子元素共享同一个 rect
            for child in children {
                render_stmt(ui, lua, child, rect, actions);
            }
        }
        ContainerKind::VBox => {
            layout_linear(ui, lua, children, rect, true, props, actions);
        }
        ContainerKind::HBox => {
            layout_linear(ui, lua, children, rect, false, props, actions);
        }
        ContainerKind::Frame => {
            // Frame 内部子元素均使用 apply_frame_props 自行定位
            for child in children {
                render_stmt(ui, lua, child, rect, actions);
            }
        }
    }
}

/// VBox / HBox 线性布局
fn layout_linear(
    ui: &mut impl UiRenderer,
    lua: &Lua,
    children: &[UiStmt],
    parent: Rect,
    vertical: bool,
    _container_props: &[viviscript_core::ast::UiProp],
    actions: &mut Vec<String>,
) {
    if children.is_empty() {
        return;
    }

    // 计算每个子元素的主轴尺寸
    let total = if vertical { parent.h } else { parent.w };
    let mut explicit: Vec<Option<f32>> = Vec::with_capacity(children.len());
    let mut explicit_sum = 0.0f32;
    let mut flex_count = 0usize;

    for child in children {
        let child_props = child_props(child);
        let size_key = if vertical { "height" } else { "width" };
        if let Some(sz) = get_f32_prop(child_props, size_key) {
            explicit.push(Some(sz));
            explicit_sum += sz;
        } else if let Some(pct) = get_percent_prop(child_props, size_key) {
            let sz = total * pct;
            explicit.push(Some(sz));
            explicit_sum += sz;
        } else {
            explicit.push(None);
            flex_count += 1;
        }
    }

    let flex_size = if flex_count > 0 {
        ((total - explicit_sum) / flex_count as f32).max(0.0)
    } else {
        0.0
    };

    let mut cursor = if vertical { parent.y } else { parent.x };

    for (child, maybe_sz) in children.iter().zip(explicit.iter()) {
        let sz = maybe_sz.unwrap_or(flex_size);
        let child_rect = if vertical {
            Rect::new(parent.x, cursor, parent.w, sz)
        } else {
            Rect::new(cursor, parent.y, sz, parent.h)
        };
        cursor += sz;
        render_stmt(ui, lua, child, child_rect, actions);
    }
}

// ── Widget 渲染 ──────────────────────────────────────────────────

fn render_widget(
    ui: &mut impl UiRenderer,
    lua: &Lua,
    kind: WidgetKind,
    value: Option<&str>,
    props: &[viviscript_core::ast::UiProp],
    rect: Rect,
    actions: &mut Vec<String>,
) {
    let resolved = value
        .map(|v| resolve_value(lua, v))
        .unwrap_or_default();

    match kind {
        WidgetKind::Text => {
            let size = get_f32_prop(props, "size").unwrap_or(24.0);
            let color = get_color_prop(props, "color").unwrap_or(Color::WHITE);
            let align = get_align_prop(props);
            ui.draw_text(&resolved, rect, color, size, align, None);
        }
        WidgetKind::Button => {
            let mut btn = Button::new(&resolved);
            if let Some(bg) = get_color_prop(props, "bg") {
                btn = btn.fill(bg);
            }
            if let Some(color) = get_color_prop(props, "color") {
                btn = btn.text_color(color);
            }
            if let Some(r) = get_f32_prop(props, "border_radius") {
                btn = btn.rounded(r);
            }
            if let Some(sz) = get_f32_prop(props, "size") {
                btn = btn.size(sz);
            }
            if btn.show(ui, rect) {
                if let Some(action) = get_string_prop(props, "action") {
                    actions.push(action);
                }
            }
        }
        WidgetKind::Image => {
            let tint = get_color_prop(props, "tint").unwrap_or(Color::WHITE);
            ui.draw_image(&resolved, rect, tint);
        }
    }
}

// ── Prop 工具函数 ────────────────────────────────────────────────

fn child_props(stmt: &UiStmt) -> &[viviscript_core::ast::UiProp] {
    match stmt {
        UiStmt::Container { props, .. } => props,
        UiStmt::Widget { props, .. } => props,
    }
}

/// 根据 Frame/ZBox 的 x/y/width/height prop 裁剪出子 rect
/// 其他容器/widget 直接使用父 rect（布局由父容器控制）
fn apply_frame_props(parent: Rect, props: &[viviscript_core::ast::UiProp]) -> Rect {
    let x = resolve_coord(props, "x", parent.x, parent.w);
    let y = resolve_coord(props, "y", parent.y, parent.h);
    let w = resolve_size(props, "width", parent.w);
    let h = resolve_size(props, "height", parent.h);
    Rect::new(x, y, w, h)
}

fn resolve_coord(props: &[viviscript_core::ast::UiProp], key: &str, default: f32, total: f32) -> f32 {
    if let Some(v) = get_prop_val(props, key) {
        if v.ends_with('%') {
            if let Ok(n) = v.trim_end_matches('%').parse::<f32>() {
                return total * n / 100.0;
            }
        } else if let Ok(n) = v.parse::<f32>() {
            return n;
        }
    }
    default
}

fn resolve_size(props: &[viviscript_core::ast::UiProp], key: &str, parent_size: f32) -> f32 {
    if let Some(v) = get_prop_val(props, key) {
        if v.ends_with('%') {
            if let Ok(n) = v.trim_end_matches('%').parse::<f32>() {
                return parent_size * n / 100.0;
            }
        } else if let Ok(n) = v.parse::<f32>() {
            return n;
        }
    }
    parent_size
}

fn get_prop_val<'a>(props: &'a [viviscript_core::ast::UiProp], key: &str) -> Option<&'a str> {
    props.iter().find(|p| p.key == key).map(|p| p.val.as_str())
}

fn get_f32_prop(props: &[viviscript_core::ast::UiProp], key: &str) -> Option<f32> {
    get_prop_val(props, key).and_then(|v| v.parse().ok())
}

fn get_percent_prop(props: &[viviscript_core::ast::UiProp], key: &str) -> Option<f32> {
    get_prop_val(props, key).and_then(|v| {
        v.trim_end_matches('%').parse::<f32>().ok().map(|n| n / 100.0)
    })
}

fn get_string_prop(props: &[viviscript_core::ast::UiProp], key: &str) -> Option<String> {
    get_prop_val(props, key).map(|v| v.to_string())
}

fn get_align_prop(props: &[viviscript_core::ast::UiProp]) -> Alignment {
    match get_prop_val(props, "align") {
        Some("center") => Alignment::Center,
        Some("right") | Some("end") => Alignment::End,
        _ => Alignment::Start,
    }
}

/// 解析颜色 prop，格式支持 "#rrggbb" 和 "#rrggbbaa"
fn get_color_prop(props: &[viviscript_core::ast::UiProp], key: &str) -> Option<Color> {
    let val = get_prop_val(props, key)?;
    let hex = val.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color::rgba(r, g, b, a))
        }
        _ => None,
    }
}

/// 解析 widget value：若形如 `f.xxx` 或 `sf.xxx`，则从 Lua 求值
fn resolve_value(lua: &Lua, val: &str) -> String {
    let is_dynamic = val.starts_with("f.") || val.starts_with("sf.");
    if is_dynamic {
        lumina_core::lua_glue::eval_string(lua, val)
    } else {
        val.to_string()
    }
}
