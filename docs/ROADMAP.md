# Roadmap

Milestones are not complete until their acceptance criteria have been tested with a matching Minecraft Java client.

1. **Foundation / status**: frame codec, status response and ping/pong. CI tests, lint, x86_64 release build pass; deployed binary was tested over external TCP. Matching Minecraft client test is outstanding.
2. **Login and connection**: current code parses Login Start and explicitly disconnects. Authentication, encryption/compression, session acceptance, Configuration, and successful Login remain unimplemented. The plan is an independently written Rust implementation using the official 26.3 server JAR and data as local behavioral/schema references, not a Java-to-Rust decompilation or an alternate server project.
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
