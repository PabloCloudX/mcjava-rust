# Implemented protocol scope

Target: Java Edition 26.3, protocol 777. See [`research/protocol-26.3.md`](research/protocol-26.3.md).

## Implemented and tested

- TCP frame length VarInt, bounded to three bytes and 1 MiB.
- Handshake packet (`0x00`): protocol version, host string (valid UTF-8 and at most 255 UTF-16 code units), port, and next state. Status accepts any protocol; Login requires protocol 777.
- Status Request (`0x00`) and Status Response (`0x00`) with version name/protocol and zero online/max players.
- Ping (`0x01`) and byte-for-byte Pong echo with 8-byte timestamp payload.
- Login Start (`0x00` serverbound in Login state) with username byte length 1-16, UTF-8, ASCII `[A-Za-z0-9_]` charset, and a 16-byte UUID.
- Login Disconnect uses a JSON text component with escaping for quotes, backslashes, and control characters.
- Configuration Disconnect uses a Text Component encoded as an NBT String Tag, matching the 26.3 packet codec.
- On GitHub Actions: Rust unit tests, lint, and x86_64 release build. On VPS: downloaded release binary passed a raw TCP status and ping/pong exchange.

## Login request handling

The server rejects Login handshakes whose protocol is not 777. It parses Login Start and validates the username byte length, UTF-8, ASCII username charset, and the 16-byte UUID. Invalid Login Start packets receive Login Disconnect (`0x00`) with a generic reason.

If `MC_OFFLINE_MODE=1` is explicitly set, the server sends Login Success (`0x02`) with the submitted name/UUID and an empty properties list, accepts Login Acknowledged (`0x03`), then sends Configuration Disconnect (`0x02`) because registries and gameplay are not implemented. The Configuration Disconnect reason is an NBT String Tag text component, not JSON; Login Disconnect remains JSON. Offline login is unauthenticated and spoofable. By default, it is disabled and Login Disconnect is returned.

An offline bind to a non-loopback address is refused unless `MC_ALLOW_PUBLIC_OFFLINE=1` is also set. With the override, the server warns in the console and continues. The override is intended only for a trusted private network.

## Not implemented

Online authentication, encryption, compression, online Login Success, Configuration, Play, registry synchronization, client join testing, player connection, chunks, and gameplay. A protocol number in the status JSON is not a claim that those protocol states work.
