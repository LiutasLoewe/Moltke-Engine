# First Prototype: World Map Foundation

**Status:** Initial implementation  
**Target editor/runtime:** Godot 4.7.x (the project was scaffolded against the 4.7 stable series)

## What exists

- A Godot project with a runnable demo scene configured as the main scene.
- A renderer-independent data model for provinces and regions.
- Province adjacency stored as province IDs.
- Validation for missing IDs, duplicate IDs, invalid region references, invalid adjacency references, self-adjacency, one-way adjacency, and negative population values.
- A small six-province sample world stored in JSON.
- A separate JSON file for the demo's 2D tile layout. This is intentionally not part of the simulation model.
- A clickable 2D demo that reads the shared world data and displays province information.
- A headless test script for the world data model.

## Run the demo

1. Install the Godot 4.7 stable editor.
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

- A polygon-based 2D province renderer.
- A 3D terrain or globe renderer.
- Geographic coordinate systems and projection conversion.
- The general-purpose module manager and public module API.
- Map editing and scenario editing tools.
- Save-game format and simulation tick processing.

This prototype is a first vertical slice, not a complete strategy engine.
