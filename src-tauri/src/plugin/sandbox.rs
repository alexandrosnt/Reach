use mlua::{Lua, Result as LuaResult};

/// Create a sandboxed Luau VM with dangerous globals removed.
pub fn create_sandbox() -> LuaResult<Lua> {
    let lua = Lua::new();

    // Set memory limit (64 MB)
    let _ = lua.set_memory_limit(64 * 1024 * 1024);

    // Remove dangerous globals
    {
        let globals = lua.globals();
        // Remove filesystem/process access
        globals.raw_remove("io")?;
        globals.raw_remove("loadfile")?;
        globals.raw_remove("dofile")?;

        // Restrict os table to only safe functions
        if let Ok(os_table) = globals.get::<mlua::Table>("os") {
            os_table.raw_remove("execute")?;
            os_table.raw_remove("exit")?;
            os_table.raw_remove("remove")?;
            os_table.raw_remove("rename")?;
            os_table.raw_remove("tmpname")?;
            os_table.raw_remove("getenv")?;
        }
    }

    Ok(lua)
}
