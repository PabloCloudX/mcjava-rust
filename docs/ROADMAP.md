# Roadmap

Milestones are not complete until their acceptance criteria have been tested with a matching Minecraft Java client.

1. **Foundation / status**: frame codec and status ping vertical slice. CI test, lint, and x86_64 release build passed at `e1e2f90`; downloaded binary passed raw TCP status/ping. Matching Minecraft client test remains outstanding.
2. **Login and connection**: protocol 777 login, authentication policy, encryption, compression, Configuration, keep-alive, disconnect.
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

This is a long-running implementation. The requested full vanilla server is not complete; a status/ping-only vertical slice is the only planned initial target. A full Java Edition server requires sustained protocol, simulation, storage, compatibility, and security work. Do not describe the project as complete or playable until actual client tests verify each relevant milestone.
