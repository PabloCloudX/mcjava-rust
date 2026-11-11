# Mojang reference data handling

The target version and official server JAR were verified from Mojang metadata. The JAR is retained only as a local research reference under `/tmp/opencode/mc26.3/server.jar`; extracted files, where present, are private, ignored local inputs under `.local/` and are not committed or uploaded as CI artifacts.

The executable currently does **not** load the JAR or extracted data at runtime. Its status response and protocol constants are Rust source. Do not describe this as “powered by the original JAR” or claim full compatibility.

Mojang's [EULA](https://www.minecraft.net/en-us/eula) permits installing the Java server version to host online play, but also restricts distribution of Mojang-made software/content. The [Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines) prohibit redistributing the game or game files and alterations. These public terms do not clearly grant a license to extract and redistribute built-in data in a replacement server. This project is not legal advice.

Conservative handling:

- Do not commit, package, or publish the official JAR or extracted Mojang data.
- Do not upload those files to GitHub Actions, releases, VPS artifacts, or third-party services.
- Keep local research extracts ignored by Git and remove them if no longer needed.
- Any future runtime loader must read operator-provided local files, verify the expected Minecraft version, enforce archive/decompression bounds, and never redistribute the inputs.
- Assess applicable terms and law before shipping extracted data or data-derived assets. A disclaimer does not grant a license or establish Mojang permission.

## Runtime disclaimer

The server prints that it is an independent, unofficial project and is not affiliated with, endorsed by, or contributed to by Mojang Studios or Microsoft. The current build only implements status/ping and explicit login rejection. A disclaimer does not make incomplete protocol support safe or compatible.
