# Login and Configuration research, 26.3 / protocol 777

Research references:

- [Java Edition protocol packets](https://minecraft.wiki/w/Java_Edition_protocol/Packets), current protocol reference says 26.3 / 777.
- [Protocol registries](https://minecraft.wiki/w/Java_Edition_protocol/Registries), synchronized registry and required-entry notes.
- Official versioned data inventory: [server-data-inventory.md](server-data-inventory.md).

Packet IDs are state- and direction-specific.

## Login

- Clientbound Login Disconnect is packet `0x00` with a JSON text component.
- Clientbound Configuration Disconnect is packet `0x02` with a Text Component encoded as NBT. Plain text is an unnamed String Tag (`0x08` + modified UTF-8 length/body), not JSON.
- Serverbound Login Start is packet `0x00`, username string and 16-byte UUID.
- Clientbound Login Success is packet `0x02` with profile UUID, username, property array, and session UUID as described by the current packet table.
- Serverbound Login Acknowledged is packet `0x03`, no fields; transitions into Configuration.
- Online mode requires the encryption request/response exchange and Mojang session authentication. Do not send unauthenticated Login Success while claiming online mode.

The code validates Login Start and protocol 777, answers Login Disconnect by default, and sends offline Login Success only when `MC_OFFLINE_MODE=1` is explicitly enabled. It does not implement online authentication.

## Configuration

The current 26.3 packet reference lists serverbound Known Packs `0x07` and Acknowledge Finish Configuration `0x03`; clientbound Registry Data `0x07`, Finish Configuration `0x03`, Feature Flags `0x0D`, Update Tags `0x0E`, and Known Packs `0x0F`. IDs are state- and direction-specific. Confirm every payload against a pinned 26.3 reference and captured vanilla behavior before implementing.

Registry Data contains one packet per registry. Its ordered entries assign numeric IDs; each entry has an identifier and optional NBT payload. If NBT is omitted, the client must obtain the entry from a selected known pack. Listing a pack does not imply every registry entry exists; every used entry still must be listed. Update Tags must be fully sent and cannot be resolved from known packs.

For initial Play acceptance, reference requirements include a dimension type, `minecraft:plains` biome, and required damage types. Finish Configuration also requires referenced registries and tags, including required banner pattern, block transformer, damage type, instrument, jukebox song, trim material, and chicken variant data. Therefore empty registries/tags are not a defensible shortcut.

## Initial progression plan

1. Keep status and login-disconnect smoke test.
2. Build independent offline profile support behind explicit offline-mode configuration; default authentication policy must be explicit and secure.
3. Serialize clientbound Login Success and handle Login Acknowledged.
4. Build bounded Configuration state machine and parse known-pack negotiation.
5. Convert actual local 26.3 vanilla data to protocol Registry Data NBT and Update Tags. Do not commit Mojang data files.
6. Send Finish Configuration and validate client acknowledgment.
7. Implement Play Login only after required registry IDs and payloads are correct.
8. Add chunk transmission and a real-client join test before calling the server playable.

This is a research plan, not delivered functionality or a claim that the packet list alone has been independently verified against a vanilla client.
