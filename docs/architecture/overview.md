# Architecture

## Direction

Moltke is an independent, genre-agnostic strategy simulation engine written in Rust. Games define whether their entities represent countries, factions, corporations, species, provinces, or star systems. Engine state is authoritative; renderers and tools consume snapshots and submit commands.

## Workspace layers

1. `moltke-core` owns integer ticks, ordered commands, world state, generic tagged entities with typed component fields, graph primitives, fixed-point stats/modifiers, event runtime state, saves, replay, and diagnostic state hashes.
2. `moltke-content` parses TOML, resolves package dependencies, mounts layered virtual files, resolves localization keys, validates a stable data-defined system schedule across fixed tick phases, and fingerprints the effective content tree.
3. `moltke-modding` runs isolated Lua 5.4 chunks against read-only simulation snapshots and converts returned tables into typed commands.
4. `moltke-events` validates data-defined triggers/options and schedules open/resolve commands. Pending choices and cooldowns live in core save state.
5. `moltke-map` converts RGB region rasters and heightmaps to core graphs and provides deterministic seeded grid generation.
6. `moltke-gameplay` contains optional diplomacy, economy, population, and military-movement state. `Campaign::advance_tick` plans fallible economy work, evaluates events, advances core commands, then commits gameplay phases. `moltke-ai` ranks independent utility candidates and emits stable command sequences.
7. `moltke-presentation` builds ordered, immutable map snapshots from a generic graph and explicit tag-driven map-mode definitions. Its typed UI values and bounded nested tooltips are renderer-neutral; frontend applications still own drawing and widget behavior.
8. `moltke-runtime` binds scheduled IDs to compiled Rust systems. Each system reads the same immutable snapshot, uses a random stream derived from the prior state/tick/system ID, and returns typed commands; the host queues the complete batch before advancing one tick.

## Tick and determinism contract

- `Simulation::step` advances exactly one integer tick. `FixedStepClock` converts elapsed wall time to due ticks, scales speed, and supports pause; it is host pacing state and is not saved.
- Commands are ordered by tick, player ID, and sequence. Player ID zero is reserved for deterministic engine-generated commands such as event openings.
- Runtime systems execute in the validated `moltke-content` schedule. Each system has a fixed sequence range, a separate reproducible random stream, and a bounded command budget. A planning error leaves both the tick and command queue unchanged.
- Authoritative values use integers or fixed point. Graph path costs are precomputed before search; pathfinding does not call Lua.
- Ordered collections and explicit tie-breaking prevent hash iteration or thread completion order from changing decisions.
- Lua receives a fresh VM for each simulation command chunk. Its snapshot is read-only, implicit random functions are removed, and returned commands are typed before they can be scheduled.
- Content fingerprints cover normalized paths, winning package source IDs, and file bytes in sorted order. They identify mismatched content but are explicitly non-cryptographic.
- AI workers score immutable candidate inputs. Results are sorted before a host schedules commands; workers do not mutate simulation state.
- The SplitMix64 algorithm and save/replay format are compatibility contracts. Save format 6 reads formats 3 through 5 with defaults for newer fields and writes the upgraded schema.
- `CanonicalStateHash` hashes serialized authoritative world state with FNV-1a-64. It is a replay/desync diagnostic checksum, not a cryptographic integrity mechanism.

## Current limits

The current implementation is a foundation, not a complete 1.0 engine. `RuntimeHost` executes generic core systems; `Campaign::advance_tick` still owns event and gameplay orchestration, and those modules need adapters before they can participate in the general schedule. A GPU renderer, widget toolkit, editor and observer apps, reference-game content, stress qualification, and network transport remain open roadmap work.
