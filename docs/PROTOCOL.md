# Implemented protocol scope

Target: Java Edition 26.3, protocol 777. See [`research/protocol-26.3.md`](research/protocol-26.3.md).

## Implemented and tested

- TCP frame length VarInt, bounded to three bytes and 1 MiB.
- Handshake packet (`0x00`): protocol version, host string (valid UTF-8 and at most 255 UTF-16 code units), port, and next state. Status accepts any protocol; Login requires protocol 777.
- Status Request (`0x00`) and Status Response (`0x00`) with version name/protocol, max 20 players, and a live online count for clients that reached Play.
- Ping (`0x01`) and byte-for-byte Pong echo with 8-byte timestamp payload.
- Login Start (`0x00` serverbound in Login state) with username byte length 1-16, UTF-8, ASCII `[A-Za-z0-9_]` charset, and a 16-byte UUID.
- Login Disconnect uses a JSON text component with escaping for quotes, backslashes, and control characters.
- Configuration Disconnect uses a Text Component encoded as an NBT String Tag, matching the 26.3 packet codec.
- Optional Configuration/Play data when `MC_CAPTURE_DIR` contains `config_before_known.bin`, `config_known_packs.bin`, `registry.bin`, `tags.bin`, and `play_packets.bin`. The Configuration phase handles Client Information, Known Packs, Keep Alive/Ping, and Acknowledge Finish Configuration. The Play phase sends the captured Login and initial chunk packets, assigns a unique entity ID, registers the player in the player list, sends Add Entity to peers, parses serverbound movement, broadcasts entity movement, updates the chunk cache center on chunk crossings, and sends Keep Alive every 10 seconds. Data comes from a local vanilla 26.3 capture and is not committed.
- On GitHub Actions: Rust unit tests, lint, x86_64 binary build, and Linux/Windows cdylib DLL build with a C-ABI export smoke check.

## Login request handling

The server rejects Login handshakes whose protocol is not 777. It parses Login Start and validates the username byte length, UTF-8, ASCII username charset, and the 16-byte UUID. Invalid Login Start packets receive Login Disconnect (`0x00`) with a generic reason.

If `MC_OFFLINE_MODE=1` is explicitly set, the server sends Login Success (`0x02`) with the submitted name/UUID and an empty properties list, then accepts Login Acknowledged (`0x03`). With a data directory it enters Configuration, negotiates Known Packs, sends registry/tags data, accepts Acknowledge Finish Configuration, and sends the Play Login/chunk packets. Play supports serverbound movement `0x1E`/`0x1F`/`0x20`/`0x21`, clientbound entity movement `0x37`/`0x39`, player info update `0x47`, player info remove `0x46`, Add Entity `0x01`, Remove Entities `0x4E`, Set Chunk Cache Center `0x60`, and Keep Alive `0x2D`/`0x1C`. Without a capture it sends Configuration Disconnect (`0x02`); that reason is an NBT String Tag text component, not JSON. Login Disconnect remains JSON. Offline login is unauthenticated and spoofable. By default, it is disabled and Login Disconnect is returned.

An offline bind to a non-loopback address is refused unless `MC_ALLOW_PUBLIC_OFFLINE=1` is also set. With the override, the server warns in the console and continues. The override is intended only for a trusted private network.

## Not implemented

Online authentication, encryption, compression, online Login Success, a registry encoder from 26.3 data, server-generated chunks, gameplay, block interaction, inventory, chat, commands, and full server console log/message parity. The captured replay is a fixed client-visible slice, not a general Configuration/Play implementation. A protocol number in the status JSON is not a claim that those protocol states work generally.
