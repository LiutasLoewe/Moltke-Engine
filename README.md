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

See [docs/architecture/overview.md](docs/architecture/overview.md) for the architecture plan.

## First prototype

The repository now contains the first implementation slice:

- a Godot 4.7 project and demo scene;
- a shared data model for provinces, regions, and province adjacency;
- JSON sample-world data with validation;
- a separate 2D demo layout and clickable province viewer;
- a headless test script for the world-data model.

See [docs/development/first-prototype.md](docs/development/first-prototype.md) for run instructions and current limitations.

## Project status

Moltke-Engine is in early prototype development. The initial world-map data foundation and simple 2D demo are present, but the production 2D renderer, 3D renderer, general module manager, and map editor are not implemented yet. The architecture documentation describes design goals where implementation is still pending.

## License

Moltke-Engine is licensed under the Mozilla Public License 2.0 (MPL-2.0). See [LICENSE](LICENSE).
