pub mod types;
pub mod api;

pub use types::{CommandBuffer, LuaCommand};

use std::path::Path;
use mlua::{Lua, LuaSerdeExt, Table};
use log::{error, info};
use lumina_shared::config;
use crate::config::SystemConfig;
use crate::typewriter_bridge::TypewriterBridge;

/// 初始化 Lua 环境：设置 package.path、注册 `lumina.*` API，返回命令缓冲区。
pub fn init_lua(lua: &Lua, tw_bridge: &TypewriterBridge) -> CommandBuffer {
    let cmd_buffer = CommandBuffer::new();

    let sys_cfg: SystemConfig = config::get("system");
    let script_root = Path::new(&sys_cfg.script_path);

    let root_str = script_root.to_string_lossy();
    let custom_path = format!(
        "{}/?.lua;{}/?/init.lua",
        root_str, root_str
    );

    let globals = lua.globals();

    if let Ok(package) = globals.get::<Table>("package") {
        let current_path: String = package.get("path").unwrap_or_default();
        let new_path = format!("{};{}", current_path, custom_path);
        if let Err(e) = package.set("path", new_path.clone()) {
            error!("Failed to set Lua package.path: {}", e);
        } else {
            info!("Lua package.path updated to include: {}", custom_path);
        }
    }

    if globals.get::<Table>("f").is_err() {
        let f_table = lua.create_table().unwrap();
        globals.set("f", f_table).unwrap();
    }

    if globals.get::<Table>("sf").is_err() {
        let sf_table = lua.create_table().unwrap();
        globals.set("sf", sf_table).unwrap();
    }

    let rust_log = lua.create_table().unwrap();
    let lumina = lua.create_table().unwrap();
    
    api::log::register(lua, &rust_log).expect("Failed to register lua log");
    api::system::register(lua, &lumina, &cmd_buffer).expect("Failed to register system API");
    api::audio::register(lua, &lumina, &cmd_buffer).expect("Failed to register audio API");
    api::visual::register(lua, &lumina, &cmd_buffer).expect("Failed to register visual API");
    api::ui::register(lua, &lumina, &cmd_buffer).expect("Failed to register UI API");
    api::typewriter::register(lua, &lumina, tw_bridge).expect("Failed to register typewriter API");

    globals.set("_rust_log", rust_log).expect("Failed to set rust_log");
    globals.set("lumina", lumina).expect("Failed to set Lumina engine");
    cmd_buffer
}

/// 在 Lua 中求值布尔表达式，出错时返回 `false`。
pub fn evel_bool(lua: &Lua, expr: &str) -> bool {
    let chunk = format!("return {}", expr);

    lua.load(&chunk).eval::<bool>().unwrap_or_else(|e| {
        error!("Lua eval error for condition '{}': {}", expr, e);
        false
    })
}

/// 将 JSON 数据注入 Lua 全局 `f` 表（游戏变量）。
pub fn inject_vars(lua: &Lua, data: &serde_json::Value) {
    let globals = lua.globals();

    match lua.to_value(data) {
        Ok(lua_val) => {
            if lua_val.is_nil() {
                globals.set("f", lua.create_table().unwrap()).unwrap();
            } else {
                globals.set("f", lua_val).unwrap();
            }
        },
        Err(e) => error!("Failed to inject vars to Lua: {}", e),
    }
}

/// 从 Lua 全局 `f` 表中提取游戏变量，序列化为 JSON。
pub fn extract_vars(lua: &Lua) -> serde_json::Value {
    let globals = lua.globals();

    if let Ok(val) = globals.get::<mlua::Value>("f") {
        serde_json::to_value(&val).unwrap_or_else(|e| {
            error!("Failed to serialize Lua 'f' table: {}", e);
            serde_json::Value::Null
        })
    } else {
        serde_json::Value::Null
    }
}

/// 在 Lua 中将表达式求值为字符串，用于文本插值 `{expr}`。
pub fn eval_string(lua: &Lua, expr: &str) -> String {
    let chunk = format!("return tostring({})", expr);

    match lua.load(&chunk).eval::<String>() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Interpolation error for '{}': {}", expr, e);
            format!("{{ERR:{}}}", expr)
        }
    }
}

/// 将 JSON 数据注入 Lua 全局 `sf` 表（跨存档持久变量）。
pub fn inject_sf(lua: &Lua, data: &serde_json::Value) {
    let globals = lua.globals();
    match lua.to_value(data) {
        Ok(lua_val) => {
            if !lua_val.is_nil() {
                globals.set("sf", lua_val).unwrap();
            }
        }
        Err(e) => error!("Failed to inject sf to Lua: {}", e),
    }
}

/// 从 Lua 全局 `sf` 表中提取持久变量，序列化为 JSON。
pub fn extract_sf(lua: &Lua) -> serde_json::Value {
    let globals = lua.globals();
    if let Ok(val) = globals.get::<mlua::Value>("sf") {
        serde_json::to_value(&val).unwrap_or_else(|e| {
            error!("Failed to serialize Lua 'sf': {}", e);
            serde_json::Value::Null
        })
    } else {
        serde_json::Value::Null
    }
}