# Gameplay Systems Roadmap

This document turns the architecture goals into implementation stages. It describes a feasible path for an independent engine; it does not claim parity with a commercial grand-strategy engine.

## Implemented foundation

The repository currently has:

- a renderer-independent province and region model with ownership IDs and an adjacency graph;
- a fixed-step daily clock with calendar progression, pause, and four simulation-speed settings;
- declarative event checks over nested simulation-state dictionaries;
- a runtime snapshot containing the clock, trigger state, and caller-owned simulation data;
- a versioned JSON save envelope for JSON-compatible snapshots;
- country tags, governments, stability, core province references, ownership validation, and dictionary-based country state serialization;
- deterministic Dijkstra routes over province adjacency with integer movement costs.

Core entry points:

- `engine/core/simulation_clock.gd`
- `engine/core/trigger_event_system.gd`
- `engine/core/simulation_runtime.gd`
- `engine/core/save_game_serializer.gd`

A trigger definition can check, for example, whether a country has low stability:

```json
{
  "id": "country.low_stability",
  "title": "Government under pressure",
  "description": "Stability has fallen below the critical threshold.",
  "once": true,
  "conditions": [
    {
      "path": "countries.NOR.stability",
      "operator": "lt",
      "value": 10
    }
  ],
  "choices": [
    { "id": "reform", "label": "Introduce reforms" },
    { "id": "suppress", "label": "Suppress opposition" }
  ]
}
```

The trigger system evaluates conditions and emits the event payload. It does not implement a pop-up renderer, execute choice effects, or run arbitrary scripts. Those require UI and validated command/effect APIs.

## 1. Core simulation and saves

**Next:** introduce typed simulation commands and apply them at tick boundaries. A command should carry a stable type ID, target simulation tick, issuing actor/player ID, sequence number, and validated payload. Modules should request changes through commands rather than mutating unrelated systems.

Keep a stable update order:

1. collect commands for the tick;
2. sort by tick, actor/player ID, and sequence;
3. validate and apply commands;
4. update enabled simulation modules in a documented order;
5. evaluate triggers and publish events;
6. record diagnostics and periodic state hashes.

The clock already separates simulation ticks from rendered frames. The save serializer currently stores JSON-compatible state and a format version; save migrations, content/module versions, and corruption recovery are still needed.

## 2. Countries and actors

Add a country registry with stable country tags, display/localization IDs, government type, ruler/leadership references where relevant, and country variables such as stability. Derive a country's owned provinces from province ownership IDs or maintain a checked index that updates with ownership changes. Store cores as stable province IDs.

Validation should catch duplicate country tags, missing owners, invalid core references, and unsupported government IDs. Province ownership remains simulation data and must not depend on map tiles or renderer objects.

## 3. Diplomacy

Build diplomacy as an optional module with explicit records for:

- directed or symmetric opinion modifiers, each with a source ID and expiry;
- alliances, guarantees, access agreements, and subject/overlord relationships;
- casus belli definitions with expiry and valid targets;
- wars with participants, war goals, score inputs, and peace terms;
- treaty effects represented as validated commands.

Keep relation queries separate from treaty application. A peace treaty should be an auditable set of effects applied at a tick boundary, not an opaque mutation of several modules.

## 4. Military and movement

Represent units by stable IDs, owner, unit type, location, strength, and movement state. Use the existing province adjacency graph for routes. Add terrain, infrastructure, ownership/access, and supply costs as explicit edge data.

The first movement foundation is implemented as deterministic Dijkstra routing over the existing adjacency graph. It accepts positive integer terrain costs, blocked provinces, and a cost limit, and breaks ties by stable province IDs. Next, add unit records and movement orders; then access rules, supply, and combat with fixed update order and saved deterministic inputs.

## 5. Economy and population

Define resources, production recipes, buildings, prices, taxes, and trade routes through validated data. Use integer or fixed-point quantities for simulation-critical values so identical inputs do not depend on floating-point ordering.

Treat population groups as optional records with size, needs, employment, culture/religion identifiers when used, and political preferences. Update high-volume systems on deliberate schedules (for example, daily movement and monthly population/economic changes), not every rendered frame. Profile representative large worlds before choosing data structures or native code.

## 6. Strategic AI and late-game performance

AI should submit the same validated commands available to a human or network client. Give every country a stable decision cadence and a bounded planning budget. Stagger country evaluations across ticks, cache expensive world queries, and re-plan only when relevant state changes.

Make any random choices from a per-game deterministic seed and stable actor IDs. Parallel work may calculate from immutable snapshots, but state changes should be merged in a stable order. Benchmark hundreds of countries and thousands of provinces before adding complex parallel execution or native extensions.

## 7. Rendering and data-driven UI

Keep the world model authoritative and renderers as consumers:

1. add province geometry and a spatial lookup/picking index to the 2D renderer;
2. add political, terrain, population, economy, and diplomatic map-mode providers;
3. render borders from ownership/geometry changes and units from simulation locations;
4. add a 3D terrain renderer that consumes the same IDs and simulation state;
5. build data-driven tables, tooltips, filters, and localization on stable query APIs.

The current clickable tile view is a data viewer, not a production map renderer. Map geometry, camera/input, animation, overlays, and UI layout should remain outside simulation state.

## 8. Modding and map tools

The documented three-level plan remains appropriate:

- JSON data packages for content and scenario data;
- a restricted Moltke Script interpreter for conditions, effects, events, and decisions;
- trusted GDScript extension modules for new developer capabilities.

Implement manifest validation and deterministic dependency ordering before script execution. Then add a parser/interpreter with allow-listed operations and an execution budget. A map editor should produce validated geometry, province IDs, adjacency, and scenario-start data that the runtime can load without editor-only dependencies.

See [Modding and Scripting Architecture](../architecture/modding-and-scripting.md).

## 9. Deterministic multiplayer

A fixed-step clock alone does not provide multiplayer determinism. Before networking, add:

- a canonical command format ordered by simulation tick, player ID, and sequence;
- deterministic random-number streams and stable iteration/sort rules;
- fixed-point or integer arithmetic for important state transitions;
- periodic state hashes and a useful mismatch report;
- replay from an initial snapshot plus the same command log.

The network layer can then transmit player commands and confirmations instead of full province/unit state each frame. Do not make cross-machine determinism claims until supported platforms have repeatable replay results.

## Delivery sequence

1. Typed commands, country records, and stable state queries.
2. Save compatibility metadata, migrations, and reproducible command replays.
3. Economy and basic country administration.
4. Diplomacy and military movement/combat.
5. Bounded, staggered AI and performance profiling.
6. Production map rendering, map modes, and data-driven UI.
7. Data packages, Moltke Script, extension lifecycle, and map/scenario tools.
8. Multiplayer command transport and out-of-sync diagnostics.
