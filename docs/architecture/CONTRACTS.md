# Architecture contracts (initial)

This document governs implementation boundaries. The project currently has no protocol/gameplay implementation; these are forward-compatible constraints, not claims of delivered features.

## Module ownership

- `network`: TCP accept loop, bounded frame codec, connection I/O and lifecycle.
- `protocol`: protocol state machine, packet decoding/encoding and status/login/play semantics. Owns packet IDs and wire-level structures.
- `server`: configuration, startup, shutdown and orchestration.
- `world`: in-memory dimensions, chunks and block-state model.
- `storage`: NBT and Anvil persistence. No implicit overwrite of existing world data.
- `entity`, `physics`, `ai`, `gameplay`, `redstone`: independent simulation domains; may depend on `world` contracts but not networking.
- `plugin`: deferred until vanilla server basics are established; native plugin code is untrusted and must not be loaded in-process without a security decision.

## Shared conventions

- Target is Minecraft Java Edition **26.3**, protocol **777**, world version **5023**. These values must be re-verified when changing targets.
- Rust is the implementation language; no JVM runtime dependency in the server executable.
- Rust standard library first. Add dependencies only when a concrete feature needs them.
- Parsing of network and disk input is fallible. Reject malformed or oversized inputs with typed errors; never panic on untrusted data.
- TCP packet framing must use explicit limits and state-specific decode paths. Queues and concurrent work must be bounded.
- The protocol layer owns packet IDs; simulation code must not depend on packet IDs.
- World coordinates use signed integer block/chunk positions; entity positions use finite floating-point values. Reject non-finite client values.
- Persistence writes must preserve existing data on failure; use temporary files and atomic replacement where supported.
- No proprietary server code or decompiled source is to be committed.

## Initial Rust boundaries

`network` may call `protocol` for decoded protocol messages and must not mutate world state directly. `protocol` may issue server actions through server-owned interfaces. `world` must not depend on Tokio or transport types. `storage` serializes stable world data and must not hold networking locks.

## Changes

Shared public types and dependency boundaries require orchestrator review before they change. Each subsystem should add the smallest test that covers a non-trivial parsing or state transition.
