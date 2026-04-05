pub(crate) mod ingame;
pub mod ui_interp;

use crate::ui::UiDrawer;
use crate::core::{AssetManager, AudioPlayer, Painter};
use lumina_core::Ctx;
use lumina_ui::Rect;
use winit::event_loop::ActiveEventLoop;

/// 屏幕切换指令
pub enum ScreenTransition {
    None,
    Quit,
}

/// 所有界面必须实现的 Trait
pub trait Screen {
    fn update(
        &mut self,
        dt: f32,
        ctx: &mut Ctx,
        el: &ActiveEventLoop,
        assets: &mut AssetManager,
        audio: &mut AudioPlayer,
    ) -> ScreenTransition;

    fn draw(&mut self, ui: &mut UiDrawer, painter: &mut Painter, rect: Rect, ctx: &mut Ctx);
}
