# Moltke Engine

Moltke is an independent, open-source foundation for grand-strategy and world-simulation games. The project is restarting as a Rust workspace, with a deterministic simulation core and Lua as its planned gameplay scripting language.

This is an early foundation, not a production-ready replacement for a mature game engine. The goal is a focused strategy engine: reusable simulation, world data, modding, and strategy-game tooling. Moltke will not reimplement every general-purpose feature of Godot.

## Current foundation

- Fixed-step simulation advanced explicitly by integer ticks; the host controls pause and wall-clock speed.
- Ordered player commands, applied in a stable order at their scheduled tick.
- Integer IDs, ordered maps, and a small seeded PRNG to support repeatable simulation.
- Initial country and province state, with ownership and stability commands.
- Versioned JSON save and load for simulation state and pending commands, with validation on load.
- A separate Lua 5.4 crate using Rust bindings, a safe standard-library subset, memory limits, and an instruction budget.

The simulation core has no renderer dependency. UI, map rendering, editor tooling, and the choice of a general-purpose rendering framework remain separate decisions.

## Workspace

- crates/moltke-core: deterministic simulation primitives, world state, commands, and saves.
- crates/moltke-modding: constrained Lua runtime foundation.
- docs/architecture/overview.md: architecture boundaries and determinism rules.
- docs/architecture/modding-and-scripting.md: the agreed data-plus-Lua modding direction.
- docs/development/roadmap.md: staged development plan.

## Build

Requires Rust 1.85 or newer. From the repository root:

```sh
cargo build --workspace
```

Lua 5.4 is built from source through the vendored mlua feature, so a separate system Lua installation is not required.

## Project status

This restart replaces the previous Godot prototype. Diplomacy, economy, populations, military, AI, event triggers, map rendering, editor tooling, and multiplayer are future work. Determinism and a renderer-independent simulation boundary are foundational constraints, not a claim that those systems are already complete.

## License

Moltke Engine is licensed under the Mozilla Public License 2.0. See [LICENSE](LICENSE).
