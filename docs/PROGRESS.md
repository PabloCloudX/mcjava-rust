# Progress tracker

| Feature | Owner | Status | Implementation / test / integration | Limitations |
|---|---|---|---|---|
| Mojang 26.3 version verification | Orchestrator | COMPLETED | Official metadata checked; server archive SHA-1 verified; protocol 777/world version 5023 recorded | Proprietary JAR is local reference only |
| TCP status and ping | Orchestrator | REVIEW_REQUIRED | Status/ping verified on VPS previously; protocol smoke test runs in CI | No matching Java client test |
| Login Start parse / rejection | Orchestrator | REVIEW_REQUIRED | Validated Login Start parser and explicit Login Disconnect; smoke test in CI | Authentication and successful login are absent |
| Big-endian NBT reader | Orchestrator | IN_PROGRESS | Compound, primitive, arrays, lists, bounds and modified UTF-8 reader; CI tests passed at commit `497e266` | No NBT writer, gzip/zlib wrappers, full Java NBT interoperability fixture suite, or protocol registry encoder |
| Configuration / registries / tags | Unassigned | NOT_STARTED | Research notes and local 26.3 data available in ignored `.local/` | No wire packets implemented |
| Play / chunk / world | Unassigned | NOT_STARTED | None | Not playable |
| VPS runtime | Orchestrator | REVIEW_REQUIRED | Current deployed VPS binary is earlier status/login-reject build | Do not deploy unverified playable claims |

No playable milestone is complete. A passing build is not client compatibility.
