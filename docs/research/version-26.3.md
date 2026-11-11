# Minecraft Java Edition 26.3 verification

Checked 2026-11-11 against Mojang's official version manifest:

- Manifest: <https://piston-meta.mojang.com/mc/game/version_manifest_v2.json>
- Exact entry: `26.3`, type `release`
- Version metadata: <https://piston-meta.mojang.com/v1/packages/9f67e10ed013deeaad20ac3c800652765d493408/26.3.json>
- Manifest metadata SHA-1: `9f67e10ed013deeaad20ac3c800652765d493408`
- Release time: `2026-09-15T11:23:02+00:00`
- Official server URL: <https://piston-data.mojang.com/v1/objects/33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar>
- Expected server JAR size: `62,294,556` bytes
- Expected server JAR SHA-1: `33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c`
- Protocol version: `777` (read from the verified JAR's `version.json`)
- World version: `5023` (read from the verified JAR's `version.json`)
- Required Java runtime for Mojang's server: Java 25. This is not a requirement for the planned native Rust server.

The official JAR was downloaded to `/tmp/opencode/mc26.3/server.jar`. Its size and SHA-1 matched the official metadata. The JAR is a Mojang proprietary reference artifact and is intentionally not included in this project.

The manifest metadata does not itself contain a protocol version. That value was confirmed in the downloaded and hash-verified JAR's `version.json`. Do not infer packet IDs or gameplay compatibility from the protocol number alone.

## Initial archive inspection

The file is a bundled server JAR with 165 ZIP entries, including `META-INF/versions.list` and `version.json`. This confirms a bundled layout, but does not by itself establish subsystem implementation details. No decompiled proprietary source is copied or published here.
