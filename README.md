# Minecraft Rust Server

Native Rust project targeting Minecraft Java Edition 26.3 (protocol 777). The target version is verified against Mojang metadata; see [`docs/research/version-26.3.md`](docs/research/version-26.3.md).

## Current scope

The current vertical slice is limited to Minecraft's TCP handshake and status exchange: it returns a server-list status response and echoes the ping payload. Login is explicitly not implemented. This is not a playable Minecraft server, and no world, chunk, movement, or gameplay compatibility is claimed.

## Build and tests

Compilation, formatting, Clippy, and tests are configured to run on GitHub Actions, not on the VPS. Push this repository to GitHub to run those workflows and download the Linux artifact from the Actions run. Local compilation is intentionally not part of the VPS workflow.

The source is published at [PabloCloudX/mcjava-rust](https://github.com/PabloCloudX/mcjava-rust). Download the current x86_64 status-only build directly with:

```sh
wget -O minecraft-rust https://github.com/PabloCloudX/mcjava-rust/releases/download/v0.1.0-status/minecraft-rust-26.3-linux-x86_64
chmod +x minecraft-rust
```

Or use `scripts/download-build.sh`. This release is a minimal status/ping build, not a playable server. ARM64 artifacts are not currently published.

## Runtime

Run the CI-produced binary with an optional listen address, for example:

```text
./minecraft-rust 127.0.0.1:25565
```

The default is `127.0.0.1:25565`, so it is only reachable locally unless you explicitly choose another bind address. The listener has only minimal status support and must not be treated as a finished production server.

## Compatibility and licensing

The official server JAR was used only as a locally stored, SHA-1-verified behavioral reference at `/tmp/opencode/mc26.3/server.jar`; it is not included here. No proprietary server code or decompiled source is distributed. See the documentation for verified facts and limitations.
