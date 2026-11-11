# Minecraft Rust Server

Independent, unofficial project. Not affiliated with, endorsed by, or contributed to by Mojang Studios or Microsoft. Minecraft is a Mojang/Microsoft product and trademark.

Native Rust project targeting Minecraft Java Edition 26.3 (protocol 777). The target version is verified against Mojang metadata; see [`docs/research/version-26.3.md`](docs/research/version-26.3.md).

## Current scope

The current vertical slice implements Minecraft's TCP handshake, status exchange, offline Login, Configuration, and the initial Play packet sequence. With `MC_OFFLINE_MODE=1` on a trusted private network, it sends offline Login Success. If a local capture is installed with `MC_CAPTURE_DIR`, it replays the vanilla Configuration packets and the initial Play Login/chunk packets, so a real client can enter the world. Without a capture it falls back to an explicit Configuration Disconnect. Offline mode does not authenticate players and allows identity spoofing. It is disabled by default. A public bind is refused unless `MC_ALLOW_PUBLIC_OFFLINE=1` is also set; even then the server prints a warning. Use that override only if you accept spoofing risk. Login also rejects protocol versions other than 777 and usernames outside the ASCII `[A-Za-z0-9_]` set. Gameplay logic, movement, and chunk streaming are not implemented beyond the captured initial packets.

## Build and tests

Compilation, formatting, Clippy, and tests are configured to run on GitHub Actions, not on the VPS. Push this repository to GitHub to run those workflows and download the Linux binary and Linux/Windows DLL artifacts from the Actions run. Local compilation is intentionally not part of the VPS workflow.

The library target builds as a C-compatible shared library:

```sh
cargo build --release --lib
```

It produces `target/release/libminecraft_rust.so` on Linux or `minecraft_rust.dll` on Windows. Exported functions are `minecraft_rust_protocol_version()` and the blocking `minecraft_rust_run(address)`. `minecraft_rust_run` accepts a NUL-terminated listen address, or NULL for `127.0.0.1:25565`, and returns 0 on clean exit or 1 on error. It is marked `unsafe` because it dereferences the C string pointer.

The source is published at [PabloCloudX/mcjava-rust](https://github.com/PabloCloudX/mcjava-rust). Download the current x86_64 status-only build directly with:

```sh
wget -O minecraft-rust https://github.com/PabloCloudX/mcjava-rust/releases/download/v0.2.0-login-reject/minecraft-rust-26.3-linux-x86_64
chmod +x minecraft-rust
```

Or use `scripts/download-build.sh`. This release is a minimal status/ping build, not a playable server. ARM64 artifacts are not currently published.

## Runtime

Run the CI-produced binary with an optional listen address, for example:

```text
./minecraft-rust 127.0.0.1:25565
```

The default is `127.0.0.1:25565`, so it is only reachable locally unless you explicitly choose another bind address. Offline login is opt-in:

```text
MC_OFFLINE_MODE=1 ./minecraft-rust 127.0.0.1:25565
```

An offline bind to a non-loopback address is refused unless you also set `MC_ALLOW_PUBLIC_OFFLINE=1`. To let clients enter Play, point `MC_CAPTURE_DIR` at a directory containing the four captured streams:

```text
MC_OFFLINE_MODE=1 MC_CAPTURE_DIR=./capture ./minecraft-rust 127.0.0.1:25565
```

The capture files are locally generated reference data and are not committed. Without them the server still serves status and login, then disconnects during Configuration. The listener must not be treated as a finished production server.

## Compatibility and licensing

The official server JAR was used only as a locally stored, SHA-1-verified behavioral reference at `/tmp/opencode/mc26.3/server.jar`; it is not included here. No proprietary server code or decompiled source is distributed. See the documentation for verified facts and limitations.
