# Implemented protocol scope

Target: Java Edition 26.3, protocol 777. See [`research/protocol-26.3.md`](research/protocol-26.3.md).

## Implemented and tested

- TCP frame length VarInt, bounded to three bytes and 1 MiB.
- Handshake packet (`0x00`): protocol version, host string (valid UTF-8 and at most 255 UTF-16 code units), port, and next state.
- Status Request (`0x00`) and Status Response (`0x00`) with version name/protocol and zero online/max players.
- Ping (`0x01`) and byte-for-byte Pong echo with 8-byte timestamp payload.
- On GitHub Actions: Rust unit tests, lint, and x86_64 release build. On VPS: downloaded release binary passed a raw TCP status and ping/pong exchange.

## Not implemented

Login, encryption, compression, Configuration, Play, registry synchronization, client compatibility testing, player connection, chunks, and gameplay. A protocol number in the status JSON is not a claim that those protocol states work.
