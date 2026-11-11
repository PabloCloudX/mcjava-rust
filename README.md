# Minecraft Rust Server

Native Rust project targeting Minecraft Java Edition 26.3 (protocol 777). The target version is verified against Mojang metadata; see [`docs/research/version-26.3.md`](docs/research/version-26.3.md).

## Current scope

The current vertical slice is limited to Minecraft's TCP handshake and status exchange: it returns a server-list status response and echoes the ping payload. Login is explicitly not implemented. This is not a playable Minecraft server, and no world, chunk, movement, or gameplay compatibility is claimed.

## Build and tests

Compilation, formatting, Clippy, and tests are configured to run on GitHub Actions, not on the VPS. Push this repository to GitHub to run those workflows and download the Linux artifact from the Actions run. Local compilation is intentionally not part of the VPS workflow.

The workspace currently has no configured GitHub remote and no GitHub CLI/authentication. Configure a repository remote and push from an authorized environment before expecting Actions artifacts.

## Runtime

Run the CI-produced binary with an optional listen address, for example:

```text
./minecraft-rust 127.0.0.1:25565
```

The default is `127.0.0.1:25565`, so it is only reachable locally unless you explicitly choose another bind address. The listener has only minimal status support and must not be treated as a finished production server.

## Compatibility and licensing

The official server JAR was used only as a locally stored, SHA-1-verified behavioral reference at `/tmp/opencode/mc26.3/server.jar`; it is not included here. No proprietary server code or decompiled source is distributed. See the documentation for verified facts and limitations.
