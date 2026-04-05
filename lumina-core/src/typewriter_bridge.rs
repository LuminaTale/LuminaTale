use std::sync::{Arc, Mutex};

#[derive(Default)]
struct TypewriterBridgeInner {
    /// 由 Lua 写入，渲染器消费
    pending_set: Option<String>,
    /// 由 Lua 写入，渲染器消费
    skip: bool,
    /// 由渲染器写入，Lua 读取
    display: String,
    /// 由渲染器写入，Lua 读取
    done: bool,
}

/// Lua 与渲染器之间共享的打字机状态桥
///
/// - Lua 通过 `lumina.typewriter.*` API 读写
/// - 渲染器的 Typewriter 动画驱动每帧同步
#[derive(Clone, Default)]
pub struct TypewriterBridge(Arc<Mutex<TypewriterBridgeInner>>);

impl TypewriterBridge {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(TypewriterBridgeInner::default())))
    }

    // ── Lua 侧 API ──────────────────────────────────────────────

    pub fn lua_set(&self, text: String) {
        if let Ok(mut g) = self.0.lock() {
            g.pending_set = Some(text);
            g.skip = false;
        }
    }

    pub fn lua_skip(&self) {
        if let Ok(mut g) = self.0.lock() {
            g.skip = true;
        }
    }

    pub fn lua_get_display(&self) -> String {
        self.0.lock().map(|g| g.display.clone()).unwrap_or_default()
    }

    pub fn lua_is_done(&self) -> bool {
        self.0.lock().map(|g| g.done).unwrap_or(true)
    }

    // ── 渲染器侧 API ─────────────────────────────────────────────

    /// 取走待设置的文本（若有）
    pub fn take_pending_set(&self) -> Option<String> {
        self.0.lock().ok().and_then(|mut g| g.pending_set.take())
    }

    /// 取走并清除跳过标志
    pub fn take_skip(&self) -> bool {
        self.0.lock()
            .map(|mut g| std::mem::replace(&mut g.skip, false))
            .unwrap_or(false)
    }

    /// 渲染器每帧更新当前显示文本和完成状态
    pub fn update_display(&self, text: String, done: bool) {
        if let Ok(mut g) = self.0.lock() {
            g.display = text;
            g.done = done;
        }
    }
}
