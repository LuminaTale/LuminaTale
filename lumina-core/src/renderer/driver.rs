use std::sync::Arc;
use mlua::Lua;
use crate::{storager, Ctx, Executor};
use crate::event::InputEvent;
use crate::manager::ScriptManager;
use crate::typewriter_bridge::TypewriterBridge;

pub struct ExecutorHandle{
    exe: Executor,
    manager: Arc<ScriptManager>,
}

impl ExecutorHandle {
    /// 创建执行器句柄：加载全局数据并从 `init` 标签启动脚本。
    pub fn new(ctx: &mut Ctx, manager: Arc<ScriptManager>) -> Self {
        let mut exe = Executor::new(manager.clone());
        exe.load_global_data();
        exe.start(ctx, "init");
        Self { exe, manager }
    }

    /// 获取打字机桥（供渲染器同步动画状态用）
    pub fn typewriter_bridge(&self) -> TypewriterBridge {
        self.exe.typewriter_bridge.clone()
    }

    /// 暴露 Lua VM 引用，供渲染器在 draw 阶段求值动态 widget 值
    pub fn lua(&self) -> &Lua {
        &self.exe.lua
    }

    /// 仅排空 Lua 命令缓冲区，不推进脚本。在 blocked_by_screen 时也需调用。
    #[inline]
    pub fn drain_commands(&mut self, ctx: &mut Ctx) { self.exe.drain_commands(ctx); }

    #[inline]
    pub fn step(&mut self, ctx: &mut Ctx) -> bool { self.exe.step(ctx) }
    
    #[inline]
    pub fn tick(&mut self, dt: f32) { self.exe.tick(dt); }

    /// 处理用户输入事件；存读档请求在此拦截，其余转发给执行器。
    #[inline]
    pub fn feed(&mut self, ctx: &mut Ctx, ev: InputEvent) {
        match ev {
            InputEvent::SaveRequest {slot} => {
                log::info!("Try to save request slot: {}", slot);

                self.exe.sync_vars_to_ctx(ctx);

                storager::save(&format!("save{}.bin", slot), ctx.clone(), self.exe.clone())
                    .unwrap_or_else(|e| log::error!("save failed: {}", e));
                self.exe.feed(InputEvent::Continue);
                log::info!("Save finished");
            }
            InputEvent::LoadRequest { slot } => {
                log::info!("Load request slot: {}", slot);
                match storager::load(&format!("save{}.bin", slot), self.manager.clone()) {
                    Ok((new_ctx, new_exe)) => {
                        *ctx = new_ctx;
                        ctx.dialogue_history.pop();

                        new_exe.sync_vars_from_ctx(ctx);

                        new_exe.load_global_data();
                        self.exe = new_exe;
                        log::info!("Load finished");
                    }
                    Err(e) => {
                        log::error!("Load failed: {:?}", e);
                    }
                }
            }
            _ => self.exe.feed(ev),
        }
    }
}