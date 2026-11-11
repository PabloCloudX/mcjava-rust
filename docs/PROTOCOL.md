# Implemented protocol scope

Target: Java Edition 26.3, protocol 777. See [`research/protocol-26.3.md`](research/protocol-26.3.md).

## Implemented and tested

- TCP frame length VarInt, bounded to three bytes and 1 MiB.
- Handshake packet (`0x00`): protocol version, host string (valid UTF-8 and at most 255 UTF-16 code units), port, and next state.
- Status Request (`0x00`) and Status Response (`0x00`) with version name/protocol and zero online/max players.
- Ping (`0x01`) and byte-for-byte Pong echo with 8-byte timestamp payload.
- On GitHub Actions: Rust unit tests, lint, and x86_64 release build. On VPS: downloaded release binary passed a raw TCP status and ping/pong exchange.

## Login request handling

The server parses Login Start (`0x00` serverbound in Login state), validating username byte length, UTF-8, and the 16-byte UUID. If `MC_OFFLINE_MODE=1` is explicitly set while bound to a non-public address, it sends Login Success (`0x02`) with the submitted name/UUID and an empty properties list, accepts Login Acknowledged (`0x03`), then sends Configuration Disconnect (`0x02`) because registries and gameplay are not implemented. Offline login is unauthenticated and spoofable. By default, it is disabled and Login Disconnect (`0x00`) is returned. With offline mode enabled on any bind address, including public IPv4/IPv6, the server warns in the console and continues.

## Not implemented

Authentication, encryption, compression, Login Success, Configuration, Play, registry synchronization, client join testing, player connection, chunks, and gameplay. A protocol number in the status JSON is not a claim that those protocol states work.
