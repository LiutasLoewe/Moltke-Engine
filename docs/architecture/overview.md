# Architecture

## Direction

Moltke is a domain-specific strategy simulation engine written in Rust. It is built around the needs of grand-strategy games: large world state, fixed simulation ticks, data-driven content, modding, and deterministic command processing.

The simulation stays independent of the eventual renderer and editor. Rendering, audio, asset import, physics, and general-purpose editor features are outside the strategy simulation core. We will evaluate Rust rendering frameworks when the map and UI needs are clearer.

## Layers

1. Simulation core — tick counter, serializable state, command queue, deterministic random stream, event and trigger scheduling.
2. Game modules — optional diplomacy, economy, military, population, politics, and AI systems operating through explicit interfaces.
3. Content and mods — structured definitions for data plus Lua for rules and event logic. Scripts submit validated simulation commands through a limited API.
4. Presentation and tools — map renderer, data-driven UI, and map/scenario editor. These consume simulation snapshots and send player commands; they do not own authoritative game state.
5. Multiplayer transport — exchanges ordered player commands and checks state hashes. Networking is not part of the initial core.

## Determinism rules

- Advance the simulation by integer ticks. Real-time frame pacing, pause, and speed controls belong to the host application.
- Give all player inputs a simulation tick and a stable order key. Never depend on network arrival order.
- Prefer integer or fixed-point quantities for authoritative state. Floating-point calculations and platform-dependent iteration must not affect lockstep outcomes.
- Use ordered collections and explicit tie-breaking whenever iteration order can affect results.
- Keep random draws in a versioned, seeded algorithm. A release that changes it must account for save and multiplayer compatibility.
- Save pending commands and all future gameplay state, not just the visible world.
- A state hash and replay format still need a separately specified, canonical byte encoding before they can support multiplayer verification.

These rules make reproducibility a design target. They do not by themselves guarantee that future modules, Lua scripts, or platform builds are deterministic.

## Current boundary

The first Rust slice contains an integer tick counter (without wall-clock pacing), country/province state, ordered commands, a seeded SplitMix64 stream, and versioned JSON persistence. Calendar rules, triggers, gameplay modules, renderer, tools, and network protocol are not implemented yet.
