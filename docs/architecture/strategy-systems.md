# Genre-agnostic strategy systems

Names such as country, province, army, or population belong to game content. The core operates on stable IDs, graph nodes, integer values, typed commands, and snapshots.

## Crate boundaries

- moltke-core owns deterministic IDs, tick state, ordered commands, the graph, fixed-point values, modifiers, and saves. No renderer, image decoder, network stack, or Lua VM is required by core.
- moltke-content (planned) parses versioned data and checks references before a game starts.
- moltke-modding embeds Lua 5.4. Scripts receive snapshots and return validated command descriptions; they do not mutate core state.
- moltke-events (planned) evaluates declarative triggers, schedules events, and turns player choices into commands.
- moltke-map-import (planned) converts RGB region maps, height maps, coordinate files, and procedural generators into validated graph data.
- moltke-mod-loader (planned) resolves manifests and virtual files before loading content.
- moltke-localization (planned) resolves stable keys and locale fallback.
- moltke-ai (planned) scores immutable snapshots and emits commands.
- moltke-presentation-api (planned) exposes read-only snapshots, map modes, tooltip queries, selection, and command submission to a Rust or external renderer.

Optional gameplay crates register state, command handlers, and tick hooks. Games choose modules in their manifest; the core does not require diplomacy, populations, or combat.

## Map graph

MapGraph is a directed graph. Nodes have stable IDs, content keys, optional integer 3D positions, and tags. Edges have IDs, endpoints, tags, and positive integer base costs. Represent a two-way route using two directed edges. Land/sea provinces and star systems/hyperlanes use the same representation.

Image import maps each non-background RGB color to a stable node key, then validates adjacency. Height maps and coordinate files supply integer position and elevation values. Procedural generation must use a versioned seeded algorithm and produce the same graph from the same seed and configuration.

The implemented pathfinder is Dijkstra, which supports any nonnegative edge costs. A* can be added when a game supplies a deterministic admissible heuristic. Dynamic Lua rules must be evaluated at a controlled tick boundary into an integer edge-cost override table. Pathfinding never calls Lua. Missing override means base cost; an explicit blocked override removes the edge from consideration.

## Entities and modifiers

Gameplay objects use globally unique EntityId values. Optional game modules define components and actor concepts. StatId names data-defined values such as tax_income, fleet_capacity, or mana_regen.

ModifierSet stores source-attributed values in ModifierId order. Fixed-point values use scale 10,000. Active additive terms are summed, then multiplicative factors apply in stable ID order. Modifiers can expire at a tick. Evaluation results are cached only for the current tick; adding, removing, or expiring a modifier clears the derived cache. Cache entries are excluded from saves.

Relationships can use the same values with a pair of entity IDs as a relation key; the core does not assume that countries are the only actors. A future relation module owns that policy.

## Events and Lua

Content defines event IDs, trigger expressions, options, and localization keys. Trigger evaluation reads a stable snapshot. Mean-time-to-happen uses integer probability scheduling and the simulation random stream, never wall-clock time or floating probability.

Choosing an option validates it and emits commands for a defined tick. On-actions are typed hooks at documented simulation phases. Lua receives narrow snapshot values and can return commands through a host API. Simulation scripts cannot access files, OS, network, wall-clock time, native modules, or nondeterministic random. UI-only scripting can have a separate capability set and cannot change authoritative state.

Pop-ups, decisions, and mission/research trees are data and presentation layers over triggers, effects, and commands. The core stores IDs and schedules; the renderer decides how they look.

## Determinism boundary

Authoritative changes are ordered by tick, player, and sequence. Planned tick phases are: expire modifiers; apply due commands; run on-actions and triggers in stable ID order; schedule future events; publish a read-only snapshot. Freeze and version this ordering before gameplay modules rely on it.

Parallel AI may score independent candidates against an immutable snapshot. Sort results by stable actor and action IDs before converting them to commands. Threads never mutate authoritative state. Lua hooks affecting the simulation execute serially unless deterministic parallel behavior is proven.

Multiplayer will exchange commands and compare canonical state hashes. Session compatibility includes content, modules, Lua scripts, and PRNG versions. Rendering, caches, localization, and wall-clock pacing remain outside authoritative state.

## Mod loading and tools

The mod loader layers a base game and enabled mods in a virtual filesystem. Manifests declare IDs, versions, dependencies, load order, compatibility range, and override policy. Conflicts report both source paths. Parse and validate content before starting the simulation.

Localization uses keys rather than literal text in authoritative state. Hot reload is limited to paused single-player or editor sessions; multiplayer rules cannot silently change. The in-game console issues the same validated commands as the UI. Observer mode runs headlessly and records commands, hashes, AI decisions, and performance counters.

## Illustrative data sketches

These TOML-like snippets show the schema direction; no content loader accepts them yet.

### Medieval province

```toml
[[map.nodes]]
id = 10
key = "north_ford"
tags = ["land", "settlement"]
position = [1200, 450, 0]

[[map.edges]]
id = 70
from = 10
to = 11
base_cost = 180
tags = ["road", "river_crossing"]

[[modifiers]]
id = 900
target_entity = 10
stat = "tax_income"
kind = "additive"
value_raw = 2500
source = "building.market"
expires_at_tick = 1200
```

### Sci-fi star system

```toml
[[map.nodes]]
id = 200
key = "kepler_reach"
tags = ["star_system", "habitable"]
position = [830000, -120000, 40000]

[[map.edges]]
id = 500
from = 200
to = 201
base_cost = 24
tags = ["hyperlane", "unstable"]

[[modifiers]]
id = 1200
target_entity = 420
stat = "fleet_capacity"
kind = "multiplicative"
value_raw = 11500
source = "technology.quantum_docks"
```

The same modifier API can model a corporation's production rate or a fantasy faction's mana regeneration. Raw value 10,000 is 1.0; 11,500 is a 1.15 multiplier.

### Lua event sketch

```lua
-- Illustrative only; the event host API is not wired yet.
return {
  id = "frontier_market",
  trigger = { stat_below = { entity = 10, stat = "stability", value_raw = 2500 } },
  options = {
    { id = "charter", effects = { add_modifier = "frontier_trade" } },
    { id = "refuse", effects = { add_resource = { entity = 10, key = "coin", amount = -50 } } }
  }
}
```

Loader formats, trigger AST, resource ownership, and host API need versioned specs before mods rely on them.
