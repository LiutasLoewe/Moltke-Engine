# Development roadmap

This is a staged plan; later systems depend on a small, stable core.

## 1. Simulation foundations

- Specify tick/calendar semantics and event scheduling.
- Add deterministic triggers, event queues, and a canonical state hash.
- Define save migrations and replay files.
- Add explicit module lifecycle and state validation contracts.

## 2. World data

- Design compact province, region, adjacency, and map-geometry data.
- Add deterministic pathfinding and data validation for large maps.
- Define stable IDs and a map/scenario interchange format.

## 3. Content and mods

- Select and version the structured data format and mod manifest.
- Load Lua scripts through the constrained runtime.
- Add a narrow command-building API, diagnostics, dependency ordering, and mod compatibility checks.

## 4. Gameplay modules

Build independent modules in vertical slices: countries and government, diplomacy and wars, economy and trade, populations, military movement and supply, then strategic AI. Measure performance with representative large worlds before adding parallel execution.

## 5. Presentation and tooling

Compare Rust rendering frameworks against the map and UI requirements. Build map modes, overlays, data-driven panels and tooltips, then a map/scenario editor. Keep the simulation headless-capable.

## 6. Multiplayer

Only after deterministic command processing and canonical state hashing are stable: define command synchronization, authority and pause rules, desync reports, save/replay compatibility, and network transport.

## Scope

No single language or renderer removes every engine problem. The project will aim for clear boundaries, measurable performance, reproducible simulation, contributor-friendly Rust code, and modder-friendly Lua and data files. Each later milestone needs profiling and compatibility decisions before it can be called production-ready.
