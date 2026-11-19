# Modding and scripting

The project keeps the earlier decision to support both structured content and a scripting language.

## Content formats

- Structured text files define stable content: country and province definitions, map metadata, scenarios, resources, units, buildings, and event data.
- Lua 5.4 handles behavior that benefits from logic: scripted triggers, event effects, decisions, AI rules, and module extensions.
- Rust defines the engine and trusted host APIs. A mod should not need a Rust compiler for ordinary content or event logic.

The exact structured format and manifest schema will be chosen when the loader is implemented. Keep formats versioned and validate them with useful file-and-line diagnostics.

## Why Lua

Lua is a small, established embeddable language with a familiar syntax and a broad history in game scripting. The Rust mlua crate provides maintained bindings and supports embedding Lua 5.4. This gives mod authors a recognizable language without making the simulation core depend on a renderer or a custom language.

## Safety and multiplayer constraints

The initial runtime loads only the safe standard-library subset and caps memory and instructions per chunk. The current crate does not expose the simulation state to scripts.

When the host API is added:

- Expose narrow, documented functions that build validated commands; do not give scripts mutable access to internal state.
- Keep filesystem, process, operating-system, network, debug, and native-module access unavailable by default.
- Disable implicit math.random; provide only a simulation-seeded random stream if a gameplay use case requires it.
- Require multiplayer-relevant scripts to use integer/fixed-point rules and stable iteration. Wall-clock time, host locale, unordered iteration, and external data must not affect authoritative results.
- Include script and content hashes in saves and multiplayer compatibility checks.
- Treat mods as untrusted input. VM limits reduce risk but are not a substitute for auditing host functions and validating every command.

Lua code that does not meet deterministic multiplayer rules may still be suitable for offline UI or editor tools, but it must not alter authoritative state in a lockstep session.
