# Protocol research: Java Edition 26.3

Verified target facts: release `26.3`, protocol `777`, world version `5023`; see [version-26.3.md](version-26.3.md).

## References and confidence

- [Minecraft Wiki Java Edition protocol packet specification](https://minecraft.wiki/w/Java_Edition_protocol/Packets) identifies its 26.3 protocol reference as protocol 777. Community-maintained, not Mojang's official specification.
- [ProtocolLib PR #3670](https://github.com/dmulloy2/ProtocolLib/pull/3670) reports 26.3 packet changes: clientbound Play packets named Add Transient Block, Post Effects and Swing Animation, and serverbound Swing replaced by Punch. The PR also reports packet-ID shifts, including clientbound Keep Alive `0x2C` to `0x2D` and Join Game `0x31` to `0x32`. These are third-party implementation findings and have not been independently verified against all payloads.
- The official JAR's `version.json` establishes version metadata, not a complete packet table.

Treat packet IDs and payload layouts beyond the tested status exchange as **unverified** until checked against an appropriate authoritative/current reference and a real 26.3 client. Never transplant an ID from older versions by assumption. Login, Configuration, Play, compression, encryption, registry synchronization, and world packet details are not implemented by this initial milestone.

## Connection phases

Public packet references describe Handshaking, Status, Login, Configuration, and Play. This server currently implements only handshake dispatch to a minimal status response/ping echo; it does not implement the remaining phases.
