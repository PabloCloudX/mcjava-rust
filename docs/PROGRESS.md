# Progress tracker

| Feature | Owner | Status | Implementation / test / integration | Limitations |
|---|---|---|---|---|
| Mojang 26.3 version verification | Orchestrator | COMPLETED | Official metadata checked; server archive SHA-1 verified; protocol 777/world version 5023 recorded | Proprietary JAR is local reference only |
| TCP status and ping | Orchestrator | REVIEW_REQUIRED | Status/ping verified on VPS previously; protocol smoke test runs in CI | No matching Java client test |
| Offline Login Success | Orchestrator | COMPLETED | Login Success packet builder, protocol 777 check, username charset validation, JSON Login Disconnect, NBT Configuration Disconnect, public-bind refusal with explicit override, and wire smoke checks; CI passed at `47b1b3c` | Unauthenticated; offline mode remains opt-in |
| Protocol/login hardening | Orchestrator | COMPLETED | Ping/pong, wrong-protocol, invalid-username, public-bind refusal, public-bind override, JSON/NBT text-component encoding, and fallback/replay smoke tests; CI passed at `47b1b3c` | No matching Java client test |
| Big-endian NBT read/write | Orchestrator | IN_PROGRESS | Compound, primitives, arrays/lists, modified UTF-8; bounded document/depth/elements and round-trip tests | No compression wrappers or registry protocol encoder |
| Configuration / registries / tags | Orchestrator | IN_PROGRESS | Registry snapshot split from Play data: plugin message, feature flags, Known Packs, 32 registry-data packets, tags, Finish Configuration; smoke test reaches Play Login | Snapshot data, no JSON-to-NBT registry encoder or compression |
| Play / chunk / world | Orchestrator | IN_PROGRESS | 473 captured chunks, unique entity ID, player list, mutual Add Entity, movement broadcast, chunk center updates, 10-second Keep Alive, live online count, basic chat and commands, `MC_OPS` operators | No world generation, block interaction, inventory, full command parity, or gameplay |
| cdylib / DLL build | Orchestrator | COMPLETED | `[lib] crate-type = ["cdylib", "rlib"]`; C ABI exports `minecraft_rust_protocol_version` and `minecraft_rust_run`; Linux and Windows DLL jobs passed at `47b1b3c` | Blocking `run` export; no non-blocking lifecycle API |
| VPS runtime | Orchestrator | REVIEW_REQUIRED | Current deployed VPS binary is earlier status/login-reject build | Do not deploy unverified playable claims |

The Play slice is a captured-world network sandbox: clients can move, see each other, chat, and use basic commands inside the captured chunks, but blocks, inventory, full commands, and world generation are not implemented. A passing build is not general client compatibility.
