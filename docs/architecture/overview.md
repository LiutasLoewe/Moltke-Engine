# Architecture

## Direction

Moltke is an independent, genre-agnostic strategy simulation engine written in Rust. Games define whether their entities represent countries, factions, corporations, species, provinces, or star systems. Engine state is authoritative; renderers and tools consume snapshots and submit commands.

## Workspace layers

1. `moltke-core` owns integer ticks, ordered commands, world state, generic tagged entities with typed component fields, graph primitives, fixed-point stats/modifiers, event runtime state, saves, replay, and diagnostic state hashes.
2. `moltke-content` parses TOML, resolves package dependencies, mounts layered virtual files, and resolves localization keys.
3. `moltke-modding` runs isolated Lua 5.4 chunks against read-only simulation snapshots and converts returned tables into typed commands.
4. `moltke-events` validates data-defined triggers/options and schedules open/resolve commands. Pending choices and cooldowns live in core save state.
5. `moltke-map` converts RGB region rasters and heightmaps to core graphs and provides deterministic seeded grid generation.
6. `moltke-gameplay` contains optional diplomacy, economy, population, and military-movement state. `Campaign::advance_tick` plans fallible economy work, evaluates events, advances core commands, then commits gameplay phases. `moltke-ai` ranks independent utility candidates and emits stable command sequences.
7. `moltke-presentation` builds ordered, immutable map snapshots from a generic graph and explicit tag-driven map-mode definitions. Its typed UI values and bounded nested tooltips are renderer-neutral; frontend applications still own drawing and widget behavior.

## Tick and determinism contract

- `Simulation::step` advances exactly one integer tick. `FixedStepClock` converts elapsed wall time to due ticks, scales speed, and supports pause; it is host pacing state and is not saved.
- Commands are ordered by tick, player ID, and sequence. Player ID zero is reserved for deterministic engine-generated commands such as event openings.
- Authoritative values use integers or fixed point. Graph path costs are precomputed before search; pathfinding does not call Lua.
- Ordered collections and explicit tie-breaking prevent hash iteration or thread completion order from changing decisions.
- Lua receives a fresh VM for each simulation command chunk. Its snapshot is read-only, implicit random functions are removed, and returned commands are typed before they can be scheduled.
- AI workers score immutable candidate inputs. Results are sorted before a host schedules commands; workers do not mutate simulation state.
- The SplitMix64 algorithm and save/replay format are compatibility contracts. Save format 6 reads formats 3 through 5 with defaults for newer fields and writes the upgraded schema.
- `CanonicalStateHash` hashes serialized authoritative world state with FNV-1a-64. It is a replay/desync diagnostic checksum, not a cryptographic integrity mechanism.

## Current limits

The current implementation is a foundation, not a complete 1.0 engine. `Campaign::advance_tick` defines a fixed orchestration order for current events and gameplay systems; a content-configurable system registry and full command API for every gameplay module remain roadmap work. A GPU renderer, widget toolkit, editor and observer apps, reference-game content, stress qualification, and network transport remain open roadmap work.
