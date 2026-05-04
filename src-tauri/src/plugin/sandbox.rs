use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use mlua::{Lua, Result as LuaResult, VmState};

use crate::plugin::schema::LUA_INTERRUPT_LIMIT;

/// Per-plugin execution limits enforced by the Luau interrupt callback.
///
/// The counter is incremented on every Luau VM interrupt tick. Reset it at the
/// start of every hook/action call via [`ExecutionLimits::reset`]; if the
/// counter exceeds [`limit`] mid-execution, the interrupt returns an error
/// which Luau propagates as an aborted call.
#[derive(Clone)]
pub struct ExecutionLimits {
    counter: Arc<AtomicU64>,
    limit: u64,
}

impl ExecutionLimits {
    pub fn new(limit: u64) -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(0)),
            limit,
        }
    }

    pub fn default_limits() -> Self {
        Self::new(LUA_INTERRUPT_LIMIT)
    }

    pub fn reset(&self) {
        self.counter.store(0, Ordering::Relaxed);
    }
}

/// Create a sandboxed Luau VM with dangerous globals removed and an interrupt
/// callback that aborts runaway loops.
pub fn create_sandbox(limits: &ExecutionLimits) -> LuaResult<Lua> {
    let lua = Lua::new();

    // Memory cap (64 MB).
    let _ = lua.set_memory_limit(64 * 1024 * 1024);

    // Whitelist-by-deletion: remove dangerous globals.
    {
        let globals = lua.globals();
        globals.raw_remove("io")?;
        globals.raw_remove("loadfile")?;
        globals.raw_remove("dofile")?;

        if let Ok(os_table) = globals.get::<mlua::Table>("os") {
            os_table.raw_remove("execute")?;
            os_table.raw_remove("exit")?;
            os_table.raw_remove("remove")?;
            os_table.raw_remove("rename")?;
            os_table.raw_remove("tmpname")?;
            os_table.raw_remove("getenv")?;
        }
    }

    // CPU cap: kill runaway Lua loops that never yield.
    let counter = limits.counter.clone();
    let limit = limits.limit;
    lua.set_interrupt(move |_| {
        if counter.fetch_add(1, Ordering::Relaxed) >= limit {
            return Err(mlua::Error::external(format!(
                "Plugin execution exceeded {} interrupt ticks (runaway loop)",
                limit
            )));
        }
        Ok(VmState::Continue)
    });

    Ok(lua)
}
