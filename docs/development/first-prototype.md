# First Prototype: World Map Foundation

**Status:** Initial implementation slice  
**Target editor/runtime:** Godot 4.7.2 stable (the project targets the Godot 4.7 feature series)

## What exists

- A Godot project with a runnable demo scene configured as the main scene.
- A renderer-independent data model for provinces and regions.
- Province adjacency stored as province IDs.
- Validation for missing IDs, duplicate IDs, invalid region references, invalid adjacency references, self-adjacency, one-way adjacency, and negative population values.
- A small six-province sample world stored in JSON.
- A separate JSON file for the demo's 2D tile layout. This is intentionally not part of the simulation model.
- A clickable 2D demo that reads the shared world data and displays province information.
- A headless test script for the world data model.
- A fixed-step daily simulation clock with pause, four speed settings, date progression, and serializable clock state.
- Declarative event triggers with nested state paths, comparison operators, once/repeating behavior, and queued event payloads.
- A versioned JSON save envelope and a runtime coordinator that snapshots game state, clock state, and trigger state.

## Run the demo

1. Install Godot 4.7.2 stable or a compatible later 4.7 patch release.
2. Open the repository folder containing `project.godot`.
3. Run the project (F6/F5 from the editor, or the Run Project command).

The demo displays six province tiles. Click a tile to inspect its region and number of adjacent provinces.

## Run the data tests

From the repository root, run:

    godot --headless --path . --script res://tests/world_map_data_test.gd

The test script exits with code 0 if all checks pass and code 1 if a check fails.

## Important boundaries

The sample grid is only a demo presentation layout. Province identity, ownership, population, region membership, and adjacency are loaded into `WorldMapData`. A future 3D renderer should consume the same `WorldMapData` and provide its own geometry, camera, and picking implementation.

## Not implemented yet

- Polygon-based 2D and 3D terrain renderers, geographic projections, and animated units.
- Country registries and complete diplomacy, economy, population, military, and political simulation modules.
- Strategic AI, a command/network protocol, multiplayer synchronization, and out-of-sync diagnostics.
- A production data-driven UI and map/scenario editor.
- A package/mod loader, Moltke Script parser and interpreter, and developer extension lifecycle.
- Save migration, module-owned schema migration, and automatic backup/recovery.

This prototype is a first vertical slice, not a complete strategy engine.
