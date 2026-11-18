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

See [docs/architecture/overview.md](docs/architecture/overview.md) for the current architecture plan.

## Project status

Moltke-Engine is in the planning and early foundation stage. The architecture described in the documentation is a design target, not a claim that these systems are already implemented.

## License

Moltke-Engine is licensed under the Mozilla Public License 2.0 (MPL-2.0). See [LICENSE](LICENSE).
