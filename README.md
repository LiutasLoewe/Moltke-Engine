# Moltke Engine

Moltke is an independent Rust engine foundation for genre-agnostic strategy and world-simulation games. Its authoritative simulation is separate from presentation, and game rules are supplied through optional modules and data-driven content.

The 1.0 direction is a complete engine workflow for an original reference strategy game. Moltke does not load Paradox game data, saves, or APIs. Windows x64 is the first release target. The project is MPL-2.0.

## Implemented foundation

- Fixed integer simulation ticks and a host-side fixed-step pacer with pause, speed scaling, and bounded catch-up batches.
- Configurable calendar conversion from ticks to hours, days, months, and years.
- Stable command ordering, seeded SplitMix64 randomness, canonical state checksums, and versioned command replays.
- Generic directed map graphs with deterministic Dijkstra/A*, integer costs, and precomputed dynamic-cost overrides.
- RGB province-map and grayscale heightmap import, region adjacency construction, and seeded grid generation.
- Generic fixed-point stats and source-attributed modifiers with expiry and derived-value caching.
- Generic ordered entity storage with tags and typed component fields for integer, fixed-point, boolean, text, and entity-reference data.
- Bounded trigger expressions, saved pending event choices, repeat/cooldown rules, and deterministic command-based event effects.
- Version 6 JSON simulation saves with migration from versions 3 through 5.
- TOML package manifests, stable dependency resolution, a layered virtual filesystem, override diagnostics, and localization fallback.
- Lua 5.4 command execution against read-only country/province snapshots, isolated VMs, disabled implicit randomness, memory caps, and instruction budgets.
- Initial optional diplomacy, economy, population, and unit-movement state, plus deterministic parallel utility scoring.
- `Campaign::advance_tick` coordinates event evaluation, core commands, and enabled gameplay modules in a fixed phase order. Economy changes are planned before commit so arithmetic errors do not leave partial production or population consumption.
- Renderer-neutral graph snapshots, ordered tag-driven map modes, typed UI fields, and bounded nested tooltips.

## Workspace

- `moltke-core`: authoritative state, ticks, commands, graph, modifiers, events, saves, and replays.
- `moltke-content`: TOML parsing, package resolution, virtual files, and localization.
- `moltke-modding`: Lua 5.4 sandbox and typed command bridge.
- `moltke-events`: event catalog, trigger evaluation, choices, and effect scheduling.
- `moltke-map`: RGB/heightmap import and procedural grid generation.
- `moltke-gameplay`: optional diplomacy, economy, population, military movement, and campaign saves.
- `Campaign::advance_tick`: deterministic orchestration across events, the core tick, and gameplay modules.
- `moltke-ai`: deterministic utility scoring and command ordering.
- `moltke-presentation`: renderer-neutral map snapshots and data-bound UI payloads.

## Build

Requires Rust 1.88 or newer (the Lua 5.4 bindings set this minimum). From the repository root:

```sh
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Lua 5.4 is built from source through the vendored `mlua` feature. PNG and BMP decoding is provided by the map-import crate.

## 1.0 status

This is an early, incomplete engine foundation. Production rendering, the custom data-driven UI, map/scenario editor, full economy and logistics, combat and peace negotiation, mission/research systems, observer tooling, a complete reference game, the 8 GB memory target, and the 10-ticks/s stress target remain unfinished. Online multiplayer is planned after 1.0; the current replay and state-hash APIs are prerequisites, not a lockstep-network guarantee.

The tracked repository contains Rust engine code and no Godot project or GDScript source. Lua is reserved for mod behavior.

See [the architecture overview](docs/architecture/overview.md), [modding design](docs/architecture/modding-and-scripting.md), and [development roadmap](docs/development/roadmap.md).
