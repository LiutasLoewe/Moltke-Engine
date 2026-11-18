# Moltke-Engine Architecture Overview

**Status:** Architecture overview with implemented core foundation  
**Implementation status:** The world-data prototype and initial fixed-step clock, declarative trigger system, and versioned JSON save primitives are implemented. Other systems below remain design targets unless explicitly marked implemented.

## 1. Goals and principles

Moltke-Engine is intended to be a reusable platform for strategy and simulation games rather than one fixed game.

Its architecture follows these principles:

- **Simulation before presentation:** game state must not depend on whether it is displayed in 2D or 3D.
- **Optional gameplay systems:** diplomacy, economy, military, population, politics, research, and other systems are modules, not mandatory parts of the core.
- **Explicit interfaces:** modules communicate through documented APIs and events rather than reaching into one another's internal implementation.
- **Data-driven games:** game-specific definitions, scenarios, and balance data should live outside the engine core.
- **Incremental complexity:** start with a small working 2D map and add 3D and advanced tooling without replacing the shared world model.
- **Godot as the initial foundation:** use Godot for rendering, input, scene management, and editor/runtime infrastructure while keeping simulation concepts as independent as practical.

## 2. High-level layers

```text
Moltke-Engine
|
+-- Core
|   +-- Simulation lifecycle
|   +-- Time and turn/tick scheduling
|   +-- Event system
|   +-- Data and save management
|   +-- Module manager
|
+-- World Model
|   +-- Provinces and regions
|   +-- Adjacency and borders
|   +-- Geography and terrain data
|   +-- Coordinate systems and map geometry
|   +-- World queries
|
+-- Presentation
|   +-- 2D map renderer
|   +-- 3D world renderer
|   +-- Camera and picking
|   +-- Map modes and overlays
|   +-- Shared UI presentation
|
+-- Gameplay Modules
|   +-- Economy
|   +-- Diplomacy
|   +-- Military
|   +-- Population
|   +-- Politics
|   +-- Research
|   +-- Custom game modules
|
+-- Tools
    +-- Map and province editor
    +-- Scenario editor
    +-- Module inspection and validation
```

The diagram describes logical responsibilities. It does not require every area to become a separate Godot project or native library.

## 3. Core and modular gameplay systems

The core should contain only services that nearly every game needs:

- application and simulation lifecycle;
- simulation clock and deterministic update scheduling where feasible;
- event dispatch;
- shared data loading and validation;
- save/load infrastructure;
- module discovery, dependency validation, activation, and lifecycle.

Gameplay features should be optional modules. For example, a city-building game might enable economy and population but omit diplomacy and military. A grand-strategy game could enable most of the standard modules.

### Module metadata

A module manifest should eventually describe at least:

- stable module ID and display name;
- module version and compatible engine API version;
- required dependencies;
- optional dependencies;
- entry point and provided capabilities;
- data and asset locations.

Illustrative manifest (the exact schema is not yet implemented):

```json
{
  "id": "religion",
  "name": "Religion System",
  "version": "0.1.0",
  "dependencies": ["core"],
  "optional_dependencies": ["politics"],
  "entry_point": "scripts/religion.gd"
}
```

The module manager should detect missing dependencies and incompatible versions before a game starts. Dependency cycles should be rejected with a useful error. Optional dependencies must be checked at runtime before a module uses their capabilities.

Modules should communicate through public interfaces and events. A module must not assume that an unrelated optional module is installed.

## 4. Shared world and map model

The world model is the authoritative source of geographic and political identity. It must not contain renderer-specific scene nodes or visual objects.

Core concepts:

- **Province:** stable ID, ownership reference, region reference, gameplay attributes, and geographic shape reference.
- **Region:** collection of provinces and optional higher-level geographic metadata.
- **Adjacency:** explicit relationships between provinces, useful for movement, trade, and map queries.
- **Geography:** terrain type, elevation, climate, and other optional geographic attributes.
- **Geometry:** source map shapes and coordinate/projection information used by presentation systems.

A province keeps the same ID and simulation state regardless of the active renderer. 2D and 3D may use different visual geometry and coordinate transformations.

Illustrative province data:

```json
{
  "id": "province_001",
  "name": "Northland",
  "owner": "country_001",
  "region": "region_001",
  "terrain": "plains",
  "population": 125000
}
```

This is an example of the intended data model, not a finalized file format. Position and geometry should be stored in map geometry/projection data rather than assuming that one 2D coordinate pair is sufficient for every 3D representation.

## 5. 2D and 3D presentation

Each game selects its presentation mode: 2D map or 3D world. Both consume the same world model and simulation state. Supporting both does not require mixing 2D and 3D renderers in one game.

### 2D renderer

Planned responsibilities:

- render province polygons, borders, labels, and overlays;
- support pan, zoom, and map interaction;
- display political, terrain, economic, and other map modes;
- map screen positions to province IDs;
- support large maps through spatial indexing, caching, and chunking as appropriate.

### 3D renderer

Planned responsibilities:

- render terrain surfaces and elevation;
- display province boundaries and map overlays on 3D surfaces;
- support camera movement and picking;
- optionally display buildings, cities, and units;
- support a flat 3D terrain map first, with a globe as a later projection option.

### Shared renderer-facing concepts

Both renderers should use common concepts for:

- active map mode and its data source;
- selected province and region IDs;
- hover and selection results;
- map overlays and legends;
- conversion from pointer position to a world/province query.

Renderer-specific geometry, camera state, materials, and scene nodes remain outside the simulation core.

## 6. Map modes

Map modes should be data-driven or supplied by modules. A map mode defines how a particular data field or calculation is visualized, such as:

- political ownership;
- terrain;
- population;
- economic output;
- religion;
- diplomatic relationships.

A map mode should provide semantic values and a legend definition. Each renderer decides how to draw those values in its own presentation style.

## 7. Performance considerations

The architecture should leave room for:

- spatial indexes for province lookup and picking;
- chunked or tiled map rendering;
- caching of geometry and map-mode results;
- level of detail in 3D;
- instancing for repeated 3D objects;
- simulation updates that are independent of rendering frame rate;
- profiling before introducing lower-level native code.

These are design considerations, not requirements to implement all optimizations in the first prototype.

## 8. Proposed project layout

```text
Moltke-Engine/
+-- project.godot
+-- engine/
|   +-- core/
|   +-- world/
|   +-- rendering/
|   |   +-- 2d/
|   |   +-- 3d/
|   |   +-- map_modes/
|   |   +-- projections/
|   +-- modules/
|   +-- ui/
+-- editor/
|   +-- map_editor/
|   +-- province_editor/
|   +-- scenario_editor/
+-- games/
+-- examples/
+-- tests/
+-- docs/
    +-- architecture/
```

The folder layout may change as implementation begins. Keep boundaries based on responsibilities rather than creating empty folders solely to match this diagram.

## 9. Development sequence

1. Create the minimal Godot project and establish code conventions.
2. Implement the initial world data model: provinces, regions, IDs, and adjacency.
3. Build a small 2D prototype map with selection and a basic political map mode.
4. Define and test the renderer-facing interfaces.
5. Add a 3D terrain prototype using exactly the same province and world data.
6. Add module discovery, manifests, dependency checks, and event-based communication.
7. Add map editor and scenario tools.
8. Expand standard gameplay modules and document the public extension API.


## 11. Modding and scripting

Moltke uses a three-level extension strategy:

1. **Data packages** define game content through validated, human-readable files. JSON is the initial implementation format.
2. **Moltke Script** is a restricted game-facing language for conditions, effects, events, decisions, and rules. It runs through an allow-listed interpreter and is not a general-purpose systems language.
3. **Developer extension modules** add new simulation systems, map modes, editor tools, or optimized functionality. GDScript is the first planned extension route; native Godot GDExtension support can follow after the public API stabilizes.

Game packages, content packages, and executable extension modules are distinct concepts. A game enables only the modules it requires.

Simulation scripts and modules should read through query interfaces and request state changes through validated commands. The core applies commands at controlled simulation boundaries and publishes events for other systems. This supports diagnostics, repeatable tests, and a future deterministic multiplayer model.

Executable GDScript and native extensions run inside the game process and must be treated as trusted code. The restricted Moltke Script interpreter is the intended route for ordinary mods that should not have arbitrary engine access.

See [Modding and Scripting Architecture](modding-and-scripting.md) for package manifests, module lifecycle, API boundaries, trust model, and implementation milestones.

## 12. Open design questions

The following should be decided through prototypes rather than prematurely fixed:

- final map geometry and source asset format;
- supported 2D projections and whether/when to add a globe;
- final Moltke Script grammar and extension API details;
- save-game compatibility and module version migration;
- deterministic simulation requirements and multiplayer scope;
- boundaries between Godot scripts and any future native performance-critical code.


## 13. Current implementation status

| Area | Status |
|---|---|
| World model | Implemented: provinces, regions, ownership IDs, population values, adjacency, validation, and a small 2D data viewer. |
| Core clock | Implemented: a fixed daily tick, Gregorian date progression, pause, 1×/2×/4×/8× scheduling speeds, and serializable clock state. |
| Events and triggers | Implemented: JSON-like definitions, nested dictionary paths, comparisons, one-shot or repeating checks, and event payload queues. UI pop-ups and player decisions are not implemented. |
| Save/load | Implemented: versioned JSON envelope for JSON-compatible snapshots. Runtime, trigger, and game state can be assembled into one snapshot. Migration, recovery, and module-specific compatibility checks are future work. |
| Countries and actors | Initial data foundation implemented: country tags, government IDs, stability, core province IDs, ownership validation, and owned-province lookup. Actor lifecycle and political mechanics remain future work. |
| Diplomacy, units, supply, combat, economy, and population | Planned optional gameplay modules. Deterministic Dijkstra routing over province adjacency is implemented as a movement foundation. |
| Strategic AI | Planned. Deterministic tick scheduling and state boundaries are prerequisites, not an AI implementation. |
| Rendering and UI | Prototype only: six clickable tiles. Production geometry, map modes, unit animation, and data-driven tables/tooltips remain planned. |
| Modding and tooling | Architecture documented. The package loader, Moltke Script interpreter, extension lifecycle, map editor, and scenario editor remain planned. |
| Multiplayer | Planned. The current simulation can advance in fixed ticks, but there is no authoritative command log, state checksum protocol, or network transport. |

### Recommended implementation order

1. Define stable country/actor records and typed simulation commands.
2. Apply commands at tick boundaries and publish validated state-change events.
3. Add save compatibility metadata and migrations for content and module schemas.
4. Implement small optional gameplay modules, starting with country administration and economy.
5. Add movement and deterministic pathfinding over the existing adjacency graph, then military and diplomacy.
6. Add repeatable AI scheduling and profiling before optimizing late-game workloads.
7. Build production map/UI tooling and the data package pipeline on top of stable simulation APIs.
8. Use command logs and state hashes as the basis for multiplayer synchronization after single-player determinism is verified.
