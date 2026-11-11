# Official 26.3 server data inventory

Inspected locally on 2026-11-11 from the SHA-1-verified Mojang server JAR described in [version-26.3.md](version-26.3.md). Archive inspection only; no proprietary classes were decompiled or copied.

## Nested server archive

- Bundled server archive entry: `META-INF/versions/26.3/server-26.3.jar`
- Uncompressed size: 26,652,008 bytes
- Embedded version facts: release 26.3, protocol 777, world version 5023, data-pack version 121.0, build time `2026-09-15T11:20:48+00:00`

## Built-in data-pack files

The nested JAR contains 9,884 non-class files below `data/`, about 9,822,164 bytes uncompressed. Examples of useful data families:

| Family | File count | Uncompressed bytes |
|---|---:|---:|
| `data/minecraft/worldgen/` | 1,144 | 1,999,195 |
| `data/minecraft/recipe/` | 2,042 | 553,819 |
| `data/minecraft/loot_table/` | 1,447 | 1,056,224 |
| `data/minecraft/advancement/` | 1,866 | 1,313,029 |
| `data/minecraft/tags/` | 883 | 223,269 |
| `data/minecraft/structure/` | 1,511 | 4,236,847 |

These are data inputs useful for independent Rust parsers and behavioral reference. Their presence does not make them freely redistributable. Keep extracted Mojang files local and do not commit them or publish substantial copies without establishing permission under applicable Mojang terms.

## Safe inspection boundaries

- Archive names, sizes, version metadata, and data-file schemas can guide research.
- The archive also contains `net/minecraft/data/*.class` and other executable classes. These are proprietary code and are not copied, decompiled, or committed as part of this research.
- Implementations in this repository must be independently authored Rust. No JVM is required by the eventual server executable.

## Next technical use

Use the 26.3 data as local fixtures or a private oracle to understand Registry Data, NBT and worldgen inputs. Before using any extracted bytes in tests or binaries, check applicable licenses and keep the source artifacts outside Git. Protocol registry contents need a protocol-compatible encoding; their JSON source files cannot be sent directly as Configuration packets.

## References

- [Mojang Minecraft EULA](https://www.minecraft.net/eula)
- [Mojang Usage Guidelines](https://www.minecraft.net/usage-guidelines)
- [26.3 protocol and registry documentation](https://minecraft.wiki/w/Java_Edition_protocol/Registries)
