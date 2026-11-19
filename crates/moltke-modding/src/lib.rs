//! A constrained Lua 5.4 runtime for future Moltke content and gameplay rules.
//!
//! The runtime currently exposes no game-state API. Host functions should be
//! added only as validated command builders with deterministic behavior.

use mlua::{HookTriggers, Lua, Table, Value, VmState};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

const INSTRUCTION_CHECK_INTERVAL: u32 = 1_000;

pub struct LuaSandbox {
    lua: Lua,
}

impl LuaSandbox {
    /// Creates a Lua VM with mlua's safe standard libraries and a memory cap.
    pub fn new(memory_limit_bytes: usize) -> mlua::Result<Self> {
        let lua = Lua::new();
        lua.set_memory_limit(memory_limit_bytes)?;

        // A lockstep script must not depend on a VM-local random sequence.
        let math: Table = lua.globals().get("math")?;
        math.set("random", Value::Nil)?;
        math.set("randomseed", Value::Nil)?;

        Ok(Self { lua })
    }

    /// Executes one chunk with a conservative VM-instruction budget.
    ///
    /// Checks happen in blocks of up to 1,000 Lua VM instructions. Scripts can
    /// access only standard-library values and functions installed by the host.
    pub fn execute(
        &self,
        chunk_name: &str,
        source: &str,
        instruction_budget: u64,
    ) -> mlua::Result<()> {
        if instruction_budget == 0 {
            return Err(mlua::Error::RuntimeError(
                "script instruction budget must be greater than zero".to_owned(),
            ));
        }

        let interval = INSTRUCTION_CHECK_INTERVAL
            .min(instruction_budget.min(u64::from(u32::MAX)) as u32)
            .max(1);
        let allowed_checks = (instruction_budget / u64::from(interval)).max(1);
        let checks = Arc::new(AtomicU64::new(0));
        let hook_checks = Arc::clone(&checks);

        self.lua.set_hook(
            HookTriggers::new().every_nth_instruction(interval),
            move |_lua, _debug| {
                let completed = hook_checks.fetch_add(1, Ordering::Relaxed);
                if completed >= allowed_checks.saturating_sub(1) {
                    return Err(mlua::Error::RuntimeError(
                        "script instruction budget exceeded".to_owned(),
                    ));
                }
                Ok(VmState::Continue)
            },
        )?;

        let result = self.lua.load(source).set_name(chunk_name).exec();
        self.lua.remove_hook();
        result
    }
}
