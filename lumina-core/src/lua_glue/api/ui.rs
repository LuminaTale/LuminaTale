use mlua::{Lua, Table};
use crate::lua_glue::types::{CommandBuffer, LuaCommand};

pub fn register(lua: &Lua, table: &Table, cb: &CommandBuffer) -> mlua::Result<()> {
    let ui_table = lua.create_table()?;

    // lumina.ui.show_screen(id, overlay?)
    let cb_show = cb.clone();
    ui_table.set("show_screen", lua.create_function(move |_, (id, overlay): (String, Option<bool>)| {
        cb_show.push(LuaCommand::ShowScreen {
            id,
            overlay: overlay.unwrap_or(false),
        });
        Ok(())
    })?)?;

    // lumina.ui.hide_screen(id)
    let cb_hide = cb.clone();
    ui_table.set("hide_screen", lua.create_function(move |_, id: String| {
        cb_hide.push(LuaCommand::HideScreen(id));
        Ok(())
    })?)?;

    table.set("ui", ui_table)?;
    Ok(())
}
