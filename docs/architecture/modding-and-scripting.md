# Modding and Scripting Architecture

**Status:** Architecture decision / design target  
**Selected direction:** Data-driven content + Moltke Script + developer extension modules  
**Implementation status:** Planned; not implemented yet.

## 1. Goals

Moltke-Engine should serve two audiences without forcing them to use the same level of complexity:

- Content creators build and balance games using data files and editor tools, without writing engine code.
- Advanced modders express conditions, effects, events, decisions, and game rules in Moltke Script.
- Developers add new engine or game capabilities through versioned extension modules.

The system must preserve optional gameplay systems, renderer-independent simulation data, reproducible simulation steps, clear API boundaries, and explicit compatibility checks.

## 2. Three extension levels

### Level 1: Data packages

Data packages define content and configuration:

- JSON for the first implementation;
- provinces, regions, countries, units, buildings, resources, technologies, and scenarios;
- localization and balance data;
- event and decision definitions that refer to Moltke Script blocks.

JSON is the initial interchange format, not a permanent restriction. A future editor may author richer text resources and export normalized data.

Data packages are declarative: loading one must not execute arbitrary GDScript or native code.

### Level 2: Moltke Script

Moltke Script is a small game-facing language for:

- event triggers and effects;
- decisions and requirements;
- scripted modifiers and rules;
- reusable conditions and effects;
- scheduled or event-driven game logic.

It is not a replacement for GDScript or a general-purpose systems language. It describes game rules using a documented set of operations supplied by enabled game modules.

Illustrative pseudocode only; this is not the final grammar or API:

~~~text
event "festival.local_market" {
    trigger {
        province.has_tag("market_town")
        date.month == 5
    }

    effect {
        province.add_modifier("local_festival", days = 7)
        event.emit("festival.started")
    }
}
~~~

The syntax should be finalized only after a parser prototype and usability testing.

#### Runtime pipeline

1. Parse source text and report file, line, and column for errors.
2. Validate syntax and referenced identifiers.
3. Check function names, argument types, and required module capabilities.
4. Convert scripts into an internal representation.
5. Execute through an interpreter that exposes only approved Moltke operations.

Start with a straightforward interpreter over a small syntax tree. Consider bytecode only after profiling demonstrates a need.

#### Language limits

- No direct filesystem or network access.
- No arbitrary engine-object access or reflection.
- No arbitrary native calls.
- No unrestricted loops or recursion.
- Bounded execution cost/instruction budget.
- Explicit types and validated identifiers where practical.
- Deterministic operations for simulation logic.
- Versioned language specification.

Long-running work must not execute inside a simulation event handler. These requirements apply to the Moltke Script interpreter; they do not make arbitrary third-party code modules safe.

### Level 3: Developer extension modules

Extension modules add capabilities that cannot be expressed as data or game rules, for example:

- a new simulation system or specialized data model;
- a map mode or visualization provider;
- an editor panel or import/export tool;
- a specialized AI or economic model;
- optimized algorithms for performance-sensitive workloads.

The first supported extension API should use GDScript. It fits the initial Godot project and keeps the first developer workflow approachable.

After the public API stabilizes, an advanced native route can use Godot GDExtension, initially with C++ and godot-cpp. GDExtension loads native shared libraries at runtime; it is an extension mechanism, not a scripting language. Native extensions require compatible binaries and platform/version testing. See the official [Godot 4.7 GDExtension documentation](https://docs.godotengine.org/en/4.7/engine_details/engine_api/gdextension/index.html) and [GDExtension build guidance](https://docs.godotengine.org/en/4.7/tutorials/scripting/cpp/build_system/scons.html).

A native extension must never be required just to install an ordinary content mod.

## 3. Packages, modules, and games

Use distinct terms:

- **Game package:** playable game definition, selected modules, starting content, scenarios, and presentation choices.
- **Content package:** data and Moltke Script that add or change game content.
- **Extension module:** executable developer code that registers additional capabilities.
- **Module dependency:** a declared requirement between extension modules or game systems.

A game enables only the systems it needs. A game with no diplomacy module must not have hidden hard dependencies on diplomacy.

## 4. Package manifest

Every package should eventually describe:

- manifest schema version;
- stable publisher-qualified ID;
- package version and compatible Moltke API range;
- package kind;
- required and optional dependencies;
- load-order constraints only where needed;
- declared capabilities;
- content, script, and optional entry-point locations.

Illustrative JSON manifest; schema is not implemented:

~~~json
{
  "manifest_version": 1,
  "id": "org.example.market_festival",
  "version": "0.1.0",
  "kind": "content",
  "moltke_api": ">=0.1.0 <0.2.0",
  "dependencies": [],
  "optional_dependencies": ["org.moltke.economy"],
  "capabilities": ["content.define", "events.define"],
  "content": ["data/**/*.json"],
  "scripts": ["scripts/**/*.moltke"]
}
~~~

A code extension uses a separate module manifest and declares its GDScript entry point or optional native extension descriptor. Content packages do not need executable entry points.

## 5. Module lifecycle and public API

An extension module should follow an explicit lifecycle:

1. **Discover:** find its manifest and package files.
2. **Validate:** check schema, IDs, versions, declared capabilities, and dependencies.
3. **Resolve:** create a deterministic dependency order; reject missing, conflicting, or cyclic dependencies.
4. **Register:** expose services, event handlers, data types, map modes, or editor tools.
5. **Start:** initialize module-owned state after required services are ready.
6. **Run:** react to simulation phases and published events.
7. **Save/load:** serialize and restore module-owned state using a versioned schema.
8. **Stop:** release resources and unregister services.

The future API should expose narrow services rather than unrestricted engine internals:

- **WorldQuery:** read-only queries for provinces, regions, and world state.
- **CommandRegistry:** register validated commands that request state changes.
- **EventBus:** subscribe to and publish typed events.
- **DataRegistry:** register and query content definitions.
- **MapModeRegistry:** add map visualization providers.
- **UIExtensionRegistry:** add supported editor/game UI panels.
- **SaveStateRegistry:** register versioned module-owned save data.
- **Diagnostics:** report warnings, errors, and profiling information.

These names are conceptual, not implemented API symbols.

## 6. State changes, events, and determinism

Scripts and modules should not mutate arbitrary shared state directly.

1. Read a stable simulation snapshot through query interfaces.
2. Submit validated commands to the command queue.
3. Apply accepted commands at defined simulation boundaries.
4. Publish events describing changes that were actually applied.
5. Let other modules react through documented interfaces.

This separates requested changes from committed state and makes debugging, save/load, replays, and future multiplayer synchronization more manageable.

For reproducible simulation:

- use a controlled simulation clock and seeded random streams;
- process commands and events in a documented stable order;
- avoid wall-clock time, unspecified dictionary iteration order, and renderer frame rate in simulation decisions;
- keep presentation-only code out of simulation decisions;
- record enough command and random-state information for repeatable tests.

Future parallel computation should use read-only snapshots and return proposed results. The core validates and applies results in a deterministic merge phase. The first prototype does not need multithreading, but its ownership boundaries should leave room for it.

## 7. Trust and permissions

Capability declarations document which engine services a module intends to use. They help with diagnostics and compatibility, but are not an operating-system security sandbox.

Use clear trust categories:

- **Data-only packages:** no executable code.
- **Moltke Script packages:** run through the restricted interpreter and its allow-listed operations.
- **GDScript extension modules:** executable code inside the game process; load only code the player trusts.
- **Native GDExtension modules:** native shared libraries; treat as fully trusted executable code.

Do not claim arbitrary GDScript or native extensions are sandboxed. The mod manager and UI should clearly identify executable packages and their sources.

## 8. Compatibility, saves, and overrides

- Use semantic versions for packages and the public Moltke API.
- Keep manifest schema version separate from package version.
- Reject incompatible required API ranges before loading a game.
- Define explicit conflict and override rules; never rely on incidental file-load order.
- Save active package IDs/versions and module-owned state schema versions with each save.
- Provide migration hooks or clear compatibility errors when module state changes.
- Keep stable content IDs when display names are translated or changed.

## 9. Proposed package layouts

Example content package:

~~~text
market_festival/
+-- moltke.package.json
+-- data/
|   +-- modifiers.json
|   +-- decisions.json
+-- scripts/
|   +-- festivals.moltke
+-- localization/
    +-- en.json
    +-- de.json
~~~

Example developer extension:

~~~text
climate_system/
+-- moltke.module.json
+-- module.gd
+-- scripts/
+-- data/
+-- tests/
+-- native/                 # optional future GDExtension
~~~

Example game package:

~~~text
my_strategy_game/
+-- game.json
+-- enabled_modules.json
+-- content/
+-- scenarios/
+-- maps/
+-- ui/
~~~

The exact file and manifest names remain open until the package-loader prototype.

## 10. Implementation roadmap

1. **Content schema:** define manifest validation and stable IDs; continue using JSON.
2. **Package loader:** discover content packages, validate files, resolve dependencies, and produce deterministic load order.
3. **Command and event contracts:** define typed commands, event envelopes, and testable state transitions.
4. **Moltke Script prototype:** implement a tiny grammar for conditions/effects, parser diagnostics, and an allow-listed interpreter.
5. **Sample content mod:** add a small event/decision package that works without changing engine source code.
6. **GDScript module API:** add discovery, lifecycle, service registration, save-state hooks, and an example optional module.
7. **Developer tooling:** add manifest validation, script diagnostics, dependency inspection, and editor support.
8. **Native extension route:** after the public API stabilizes, prototype an optional GDExtension for a measured performance-sensitive or editor-focused feature.

## 11. Acceptance criteria

The initial public prototype is ready when:

- a data-only content package loads without executable code;
- an invalid manifest produces a useful diagnostic;
- missing, incompatible, and cyclic dependencies are rejected;
- a Moltke Script event can test a condition and submit a validated command;
- an over-budget script stops with a diagnostic;
- an optional module can be absent without breaking unrelated systems;
- a GDScript extension registers a service through the documented API;
- module-owned state saves and loads with a schema version;
- repeated test runs with the same inputs produce the same simulation result.

## 12. Out of scope for the first implementation

Do not start by building:

- a general-purpose programming language;
- arbitrary code execution inside the Moltke Script interpreter;
- a public binary ABI for native modules;
- automatic online mod downloading or a mod marketplace;
- hot-unloading arbitrary GDScript/native modules;
- a complex visual scripting editor.

Consider these only after the data package, interpreter, and extension API have stable prototypes.
