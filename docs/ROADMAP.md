# Roadmap

Milestones are not complete until their acceptance criteria have been tested with a matching Minecraft Java client.

1. **Foundation / status**: frame codec, status response and ping/pong. CI tests, lint, x86_64 release build pass; deployed binary was tested over external TCP. Matching Minecraft client test is outstanding.
2. **Login and connection**: status/login, opt-in offline Login Success, protocol 777 and username validation, JSON text-component escaping, and a public-bind refusal with explicit override. Authentication, encryption/compression, session acceptance, Configuration, and successful Play remain unimplemented. The plan is an independently written Rust implementation using the official 26.3 server JAR and data as local behavioral/schema references, not a Java-to-Rust decompilation or an alternate server project.

## Current implementation gate

NBT has a bounded big-endian reader and writer in `src/nbt.rs`; they are not yet wired into world loading or protocol registry emission. CI passed for NBT round-trip/parser tests, formatting, Clippy, and x86_64 build at commit `19a2663`. The next acceptance gate is registry NBT conversion and a real 26.3 client reaching Play, not a server-list ping.
3. **World and chunks**: registry synchronization, dimension and chunk data accepted by client.
4. **Movement**: movement, collision, synchronization, persistence.
5. **Basic gameplay**: block interaction, inventory, crafting and survival/creative mechanics.
6. **World generation**: compatibility-tested terrain and dimensions.
7. **Entities**: entity lifecycle and representative vanilla behavior.
8. **Advanced gameplay**: redstone, commands and other systems.
9. **Storage**: safe NBT/Anvil interoperability and recovery.
10. **Optimization**: measurements and benchmarks before performance claims.

The target remains Java 26.3. Do not silently change protocol targets.

## Project status

This remains a long-running implementation, not a playable Minecraft server. No client login, world, chunks, gameplay, or persistence compatibility is claimed. A full Java Edition server requires sustained protocol, simulation, storage, compatibility, and security work.
