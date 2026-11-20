# Development roadmap

## Milestone 1 — Generic map graph and modifiers (current)
- Stable directed nodes and edges, integer positions and costs, tags, graph validation, and deterministic Dijkstra.
- Integer cost overrides and blocked edges are supplied as a precomputed table; the pathfinder never calls Lua.
- Generic entities and stat keys, fixed-point additive and multiplicative modifiers, source attribution, expiry, stable evaluation, and a derived per-tick cache.
- Persist both systems in simulation saves and validate them on load.

## Milestone 2 — Tick phases, events, triggers, decisions
Freeze tick phase order and calendar adapters. Add serializable trigger expressions, threshold and scheduled checks, seeded mean-time-to-happen, on-actions, event options, and command effects. Connect Lua through snapshot-in and validated-command-out hooks. Model decisions and mission/research trees over the same trigger/effect system.

## Milestone 3 — Content pipeline and mod ecosystem
Choose a versioned data format and manifest. Add diagnostics, reference validation, stable IDs, mod dependency ordering, layered virtual filesystem and explicit override rules, localization keys and fallback, and content/script compatibility hashes. Permit hot reload only in paused single-player/editor sessions.

## Milestone 4 — Optional gameplay and deterministic AI
Add optional diplomacy and relations, conflicts, units and logistics, economy and trade, resources, populations, then utility AI. AI scores immutable snapshots, can parallelize independent calculations, then sorts outputs before command submission. Add observer runs and profiling counters.

## Milestone 5 — Presentation API, tooling, multiplayer
Define read-only snapshots and command submission for external renderers; add map modes and nested tooltip queries. Build RGB/heightmap import, coordinate editing, procedural map adapters, scenario tools, and headless validation. Specify canonical hashes, replay format, command synchronization, pause/authority, desync reports, and compatibility. Renderer stays an adapter, not a core dependency.

## Quality gates
Profile representative world sizes before parallelizing simulation updates. Version IDs, command order, save migrations, and PRNG algorithms. Keep caches derived. Replay identical commands and compare canonical hashes across supported platforms before claiming lockstep support.
