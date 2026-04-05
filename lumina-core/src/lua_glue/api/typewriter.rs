use mlua::{Lua, Table};
use crate::typewriter_bridge::TypewriterBridge;

/// 注册 lumina.typewriter.* API
///
/// 开发者用法：
/// ```lua
/// lumina.typewriter.set("你好世界")   -- 启动打字机动画
/// lumina.typewriter.skip()            -- 立即显示完整文本
/// lumina.typewriter.get()             -- 返回当前已显示的文本
/// lumina.typewriter.is_done()         -- 是否动画完毕
/// ```
pub fn register(lua: &Lua, table: &Table, bridge: &TypewriterBridge) -> mlua::Result<()> {
    let tw_table = lua.create_table()?;

    let b = bridge.clone();
    tw_table.set("set", lua.create_function(move |_, text: String| {
        b.lua_set(text);
        Ok(())
    })?)?;

    let b = bridge.clone();
    tw_table.set("skip", lua.create_function(move |_, ()| {
        b.lua_skip();
        Ok(())
    })?)?;

    let b = bridge.clone();
    tw_table.set("get", lua.create_function(move |_, ()| {
        Ok(b.lua_get_display())
    })?)?;

    let b = bridge.clone();
    tw_table.set("is_done", lua.create_function(move |_, ()| {
        Ok(b.lua_is_done())
    })?)?;

    table.set("typewriter", tw_table)?;
    Ok(())
}
