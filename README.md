# Moltke-Engine

Moltke-Engine is an independent, open-source, modular strategy and simulation engine built with Godot as its initial technical foundation.

It is designed for developers creating historical, fantasy, science-fiction, economic, political, and other simulation games. Each game can choose a 2D or 3D presentation while using the same underlying world and simulation data.

## Design goals

- **Modular simulation:** Games enable only the systems they need, such as economy, diplomacy, population, military, politics, or research.
- **Extensible architecture:** Developers can add custom modules through documented interfaces, dependencies, and events.
- **Shared world model:** Provinces, regions, ownership, population, and other simulation data are independent of visual presentation.
- **2D and 3D support:** Games can use a 2D map renderer or a 3D world renderer without duplicating simulation data.
- **Data-driven content:** Game definitions and scenarios should be configurable through data files rather than hard-coded into the engine.
- **Editor tooling:** The long-term goal includes tools for creating maps, provinces, regions, and scenarios.

## Architecture

The engine is planned around four main areas:

1. **Core:** simulation lifecycle, time, events, data management, and module management.
2. **World model:** provinces, regions, geography, map geometry, and world queries.
3. **Rendering:** separate 2D and 3D presentation layers that consume the shared world model.
4. **Modules and tools:** optional gameplay systems, game-specific content, and editor tools.

See [docs/architecture/overview.md](docs/architecture/overview.md) for the architecture plan and [docs/architecture/modding-and-scripting.md](docs/architecture/modding-and-scripting.md) for the modding, scripting, and extension-module design.

## First implementation slice

The repository contains a Godot 4.7 project and two working foundations:

- a renderer-independent world model for provinces, regions, ownership, population, and adjacency;
- a small clickable 2D world-map demo;
- a fixed-step daily simulation clock with pause controls, speed multipliers, and calendar signals;
- declarative state-path triggers that emit data events and can be surfaced as pop-ups or decisions by a UI;
- a versioned JSON save envelope for runtime, trigger, and JSON-compatible game state.

The core APIs are under `engine/core/`. See [docs/development/first-prototype.md](docs/development/first-prototype.md) and [the gameplay systems roadmap](docs/development/gameplay-systems-roadmap.md) for current capabilities and next steps.

## Project status

Moltke-Engine is in early prototype development. The world-data foundation, simple 2D demo, deterministic tick scheduler, declarative trigger evaluation, and versioned JSON save/load primitives are implemented. Country and diplomacy data models, economic and military simulation, strategic AI, production renderers, data-driven UI, mod loading, editor tooling, and multiplayer are still future work. The architecture documents label these boundaries explicitly.

## License

Moltke-Engine is licensed under the Mozilla Public License 2.0 (MPL-2.0). See [LICENSE](LICENSE).
