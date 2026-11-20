# Modding and scripting

Moltke uses both structured content and Lua 5.4. TOML defines package manifests, game data, event definitions, and UI layouts. Lua supplies behavior. Rust remains the trusted engine and host API.

## Package loading

`moltke-content` validates IDs and manifest structure, resolves transitive dependencies in deterministic topological order, and mounts packages into a virtual filesystem. Requested mod order breaks ties; later packages override earlier files. Every override records both source IDs. Directory loading is sorted and rejects symbolic links. Content paths are ASCII, slash-separated, and reject traversal segments.

Localization is key-based. Later package values override earlier ones; resolution checks the requested locale, then the game fallback locale, then returns the key so missing translations remain visible.

## Deterministic system schedules

`SystemRegistry` loads `systems.toml`, validates system identifiers and dependencies, and resolves a stable order across the fixed `input`, `events`, `commands`, `gameplay`, and `cleanup` phases. Dependencies within one phase are topologically sorted with system IDs as tie-breakers; cross-phase dependencies may only point forward. Explicit enablement replaces the file's defaults. The registry defines order only; the host still binds registered IDs to trusted Rust system implementations. See `examples/game/systems.toml`.

## Lua simulation API

`LuaSandbox::execute_commands` creates a fresh VM, installs a read-only `snapshot` table containing the current tick, countries, provinces, and ordered generic entities, and evaluates a chunk that returns an array of command tables. The host converts only supported command kinds to `CommandAction` values and assigns the target tick and deterministic sequence numbers. Callers schedule the returned vector atomically with `Simulation::schedule_commands`.

Example:

```lua
return {
  {
    kind = "set_country_stability",
    country_id = 1,
    stability_bp = 6500
  },
  {
    kind = "add_modifier",
    modifier_id = 42,
    target_entity_id = 1,
    stat = "tax_income",
    modifier_kind = "multiplicative",
    value_raw = 11000,
    source = "event.frontier_market",
    expires_at_tick = 120
  }
}
```

Supported command kinds include `add_country`, `add_province`, `transfer_province`, `set_country_stability`, `set_stat`, `add_modifier`, `create_entity`, entity tag changes, and typed component-field updates. Component values are integers, fixed-point values, booleans, text, or references to existing entities. Unknown kinds, invalid identifiers, invalid numeric values, overlong result arrays, memory exhaustion, and instruction-budget exhaustion fail before commands are returned.

`examples/scripts/generic_entities.lua` shows how a game can create a tagged actor and attach genre-specific identity and government fields without adding engine code.

## Events

`moltke-events` parses TOML event catalogs. Trigger expressions use the core `TriggerExpr` schema; event definitions provide a recipient, options, localization keys, once/cooldown policy, and command effects. An optional `mean_time_to_happen_ticks = N` applies a deterministic seeded 1/N hazard on each eligible tick after the trigger is true. The host evaluates catalog entries in sorted event-ID order, so the random stream remains stable. Opening an event and resolving a player choice are scheduled core commands, so pending choices, cooldowns, and once-only status survive saves and replay.

Example trigger encoding:

```toml
[[events]]
id = "frontier_market"
recipient = 1
once = true
trigger = { op = "stat_compare", args = { entity = 10, stat = "stability", comparison = "less", value = 2500 } }

[[events.options]]
id = "charter"
localization_key = "event.frontier_market.charter"
effects = [{ SetCountryStability = { country_id = 1, stability_bp = 6500 } }]
```

Event effects are queued in option order and share the same target tick. Failures appear in command outcomes; scripts cannot mutate the world directly.

## Safety and determinism

- Lua uses `mlua` with Lua 5.4, a memory cap, an instruction budget, and no implicit `math.random`/`math.randomseed`.
- Simulation script chunks use isolated VMs; persistent Lua globals cannot leak across runs.
- Filesystem, process, operating-system, network, debug, and native-module access are not part of the host API.
- Host code must validate every command and must not expose mutable core references.
- The current Lua snapshot is intentionally small. Mod-controlled AI, UI extensions, on-actions, hot reload, full content hashing, and multiplayer script compatibility are still unimplemented.
