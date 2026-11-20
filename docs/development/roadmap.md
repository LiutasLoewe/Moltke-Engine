# Development roadmap

## Milestone 1 — Deterministic core

**Implemented:** fixed-step simulation, host pacing clock, configurable tick-to-calendar mapping, ordered commands, seeded RNG, canonical state checksum, JSON replay/verification API, version 6 saves with versions 3–5 read compatibility, genre-neutral entities and typed component fields, generic graph, Dijkstra/A*, fixed-point modifiers, and a data-defined deterministic phase/dependency schedule registry.

**Still required:** bind scheduled IDs to registered runtime system implementations, move all authoritative updates behind command/system APIs, and measure the 100,000-node / 1-million-object performance target.

## Milestone 2 — Content and event runtime

**Implemented:** TOML parsing, game/mod manifests, deterministic transitive mod ordering, layered virtual files with override diagnostics, localization fallback, Lua 5.4 isolated command chunks, serializable pending events, bounded triggers, once/cooldown behavior, event options, and command-based effect scheduling.

**Still required:** on-actions, decision and mission/research systems, richer schema diagnostics, content/script hashes, hot reload, and comprehensive command validation against a loaded game definition.

## Milestone 3 — Maps, gameplay, and AI

**Implemented:** PNG/BMP RGB region import, grayscale heightmap reading, adjacency graph creation, seeded grid generation, initial diplomacy/relations/treaties/warscore, resource stockpiles and production rules, population needs/ideology shares, unit movement/supply consumption, campaign save bundling, deterministic parallel utility scoring, and a fixed campaign tick order coordinating event evaluation, core commands, and enabled gameplay modules. Economy tick updates are staged before inventory changes are committed.

**Still required:** full trade/logistics networks, peace negotiation and province transfers, configurable combat, government/cores/buildings, complete pops and ideology simulation, research, AI candidate generation, and simulation-integrated system registration.

## Milestone 4 — Renderer, UI, tools, and reference game

**Implemented interface:** renderer-neutral graph snapshots, ordered tag-driven map-mode styles, typed UI field values, and bounded nested tooltip data in `moltke-presentation`.

**Not implemented yet:** custom Rust 2D/3D `wgpu` renderer, simulation-fed political/terrain/diplomacy/economy map data, dynamic borders and unit visuals, widget toolkit and layout, map/scenario editor, in-game console, observer/headless app, and a complete historically themed sample campaign using independently licensed content.

## Milestone 5 — 1.0 hardening

**Not implemented yet:** 8 GB device qualification with a 5 GB engine-process ceiling, sustained 10 full ticks/s on the documented 8-core Windows reference PC, save/replay migration coverage, Windows release packaging, dependency/license inventory, and public modder/developer documentation.

Online multiplayer follows 1.0. Replays and state hashes are available as foundations; cross-machine lockstep support is not claimed.

## Release gates

- Workspace compilation and Clippy must pass on Windows CI; a Linux compile job can guard portability.
- Add deterministic scenario/replay checks for same-seed runs and different AI worker counts before claiming determinism across builds.
- Validate malformed TOML, Lua command output, map imports, and saves before loading user content.
- Run the exact stress scenario at the selected 8 GB/8-core device limit and publish hardware/build details with measurements.
- Keep the README and GitHub language classification aligned with tracked files; the repository contains no Godot project or GDScript source.
