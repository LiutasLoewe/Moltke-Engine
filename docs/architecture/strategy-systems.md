# Genre-agnostic strategy systems

Names such as country, province, army, corporation, or population belong to game content. The core supplies stable IDs, directed graph nodes/edges, integer and fixed-point values, typed commands, and read-only snapshots.

## Crate boundaries

- `moltke-core`: ticks, commands, graph, modifiers, triggers, event save state, saves, replays, and state hashes. It does not depend on a renderer, Lua VM, image decoder, or network stack.
- `moltke-content`: TOML parsing, game/mod manifests, deterministic dependency order, layered VFS, override reports, and localization lookup.
- `moltke-modding`: constrained Lua 5.4 chunks that read snapshots and return typed core commands.
- `moltke-events`: event definitions, trigger evaluation, cooldown/once policy, player options, and effect scheduling.
- `moltke-map`: PNG/BMP RGB region and grayscale heightmap import, adjacency graph construction, and seeded grid generation.
- `moltke-gameplay`: optional diplomacy, economy/production, population needs, military movement, and campaign save bundling.
- `moltke-ai`: parallel utility scoring over immutable candidates and deterministic result ordering.

Renderer/UI, editor, observer and host application crates are still to be built. Gameplay data is serializable, but a general registration API for systems and gameplay commands is still needed.

## Maps and pathfinding

`MapGraph` is a directed graph. Nodes have stable IDs, keys, optional integer 3D positions, and tags. Edges have stable IDs, endpoints, tags, and positive integer base costs. Use two directed edges for a bidirectional route. Land regions and star systems use the same representation.

`moltke-map` reads PNG/BMP RGB rasters; each non-background color maps to a node definition. The importer computes integer centroids, optional mean height, and four-neighbor directed adjacency. A seeded grid generator provides a simple reproducible procedural map. General-purpose procedural world generation and a visual map editor remain future work.

Core supports Dijkstra and A*. Movement-cost overrides are computed before path search; `None` blocks an edge. A* receives caller-supplied admissible integer lower bounds and requires zero at the goal. The pathfinder never invokes Lua.

## Entities, modifiers, and gameplay

`EntityId` identifies generic actors. `StatId` names data-defined values such as `tax_income`, `fleet_capacity`, or `mana_regen`. Fixed-point values use scale 10,000. Additive values are summed before multiplicative factors, which apply in modifier-ID order; expiry and derived per-tick caching are supported.

Initial `moltke-gameplay` state includes directional relations, treaties, wars and bounded warscore; owner/resource stockpiles, input/output production rules and population needs; plus unit stacks with path orders and per-edge supply consumption. It does not yet implement trade networks, peace terms, combat, government systems, buildings, or a complete population ideology loop.

## Events, Lua, and saves

Core trigger expressions cover tick thresholds, fixed-point stat comparisons, and all/any/not composition. TOML event catalogs define triggers and options. The host evaluates them at tick boundaries; opening an event and resolving a choice are scheduled commands, so pending choices and cooldowns are saved and replayable. Mean-time-to-happen, on-actions, decisions, missions, and research trees remain future work.

Lua scripts run in fresh VMs with memory/instruction limits, no implicit random functions, and a read-only tick/country/province snapshot. They return a bounded array of typed commands. Host code remains responsible for scheduling and handling command outcomes.

Simulation saves use versioned JSON. Version 4 adds event runtime state and reads version 3 by initializing that state empty. `Campaign` wraps the core save with optional gameplay state. Replays store a starting save, canonical ordered commands, and optional checksums. The FNV-1a-64 state checksum is for divergence diagnostics, not security.

## Determinism boundary and remaining work

The host controls wall-clock pacing; only integer ticks enter simulation. Commands are ordered by tick/player/sequence. Worker calculations read immutable input and sort results before commands are queued. Rendering, caches, locale resolution, and wall-clock speed remain outside authoritative state.

Multiplayer transport is after 1.0. Before any lockstep claim, Moltke still needs deterministic full-tick orchestration across gameplay modules, cross-build replay checks, content/module/script hashes, and desync diagnostics with a pinned compatibility contract.

The TOML map snippets in earlier design notes are schema sketches; no general node/edge TOML loader exists yet. Current map import consumes RGB/heightmap image files and an in-memory palette.
