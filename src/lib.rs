mod nbt;

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const PROTOCOL: i32 = 777;
const MAX_CONNECTIONS: usize = 128;
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static ONLINE_PLAYERS: AtomicUsize = AtomicUsize::new(0);
static NEXT_ENTITY_ID: AtomicI32 = AtomicI32::new(1);
static CAPTURE: OnceLock<Option<Capture>> = OnceLock::new();
static REGISTRY: OnceLock<Option<Registry>> = OnceLock::new();
const MAX_PACKET: usize = 1_048_576;
const PLAY_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(10);
const PLAY_KEEP_ALIVE_PACKET_ID: i32 = 0x2C;
const OFFLINE_MODE_ENV: &str = "MC_OFFLINE_MODE";
const ALLOW_PUBLIC_OFFLINE_ENV: &str = "MC_ALLOW_PUBLIC_OFFLINE";
const CAPTURE_DIR_ENV: &str = "MC_CAPTURE_DIR";

pub fn run(address: &str) -> io::Result<()> {
    let _nbt_parser = nbt::parse;
    let _nbt_writer = nbt::write;
    let _nbt_integer_lookup: fn(&nbt::Compound, &str) -> Option<i32> = nbt::Compound::get_int;
    let offline_mode = std::env::var(OFFLINE_MODE_ENV).as_deref() == Ok("1");
    let allow_public_offline = std::env::var(ALLOW_PUBLIC_OFFLINE_ENV).as_deref() == Ok("1");
    let bind_addr: SocketAddr = address.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "listen address must be an IP:port socket address",
        )
    })?;
    if public_offline_bind_is_refused(bind_addr, offline_mode, allow_public_offline) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "refusing to bind offline mode to public address {bind_addr}; set {ALLOW_PUBLIC_OFFLINE_ENV}=1 only on a trusted private network"
            ),
        ));
    }
    if offline_mode && offline_bind_is_public(bind_addr) {
        eprintln!(
            "WARNING: offline mode is unauthenticated and permits identity spoofing. Publicly reachable offline servers are not recommended."
        );
    }
    let listener = TcpListener::bind(bind_addr)?;
    eprintln!("Minecraft Rust Server status listener on {address} (protocol {PROTOCOL})");
    eprintln!(
        "Independent project. Not affiliated with, endorsed by, or contributed to by Mojang or Microsoft."
    );
    if capture().is_some() && registry().is_some() {
        eprintln!(
            "Configuration registry and Play replay enabled from {}.",
            capture_dir().display()
        );
    } else {
        eprintln!(
            "Configuration/Play data not found at {}; connections will disconnect after login.",
            capture_dir().display()
        );
    }
    eprintln!(
        "Minecraft Java Edition is a Mojang/Microsoft product. Gameplay logic is not implemented; client-visible Configuration/Play packets come from a local vanilla capture."
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed) >= MAX_CONNECTIONS {
                    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
                    drop(stream);
                    continue;
                }
                let spawn = std::thread::Builder::new().spawn(move || {
                    let _connection = ConnectionGuard;
                    if let Err(error) = serve(stream) {
                        eprintln!("connection ended: {error}");
                    }
                });
                if let Err(error) = spawn {
                    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
                    eprintln!("could not start connection handler: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn parse_handshake(handshake: &[u8]) -> io::Result<(i32, i32)> {
    let mut cursor = 0;
    if read_varint(handshake, &mut cursor)? != 0 {
        return Err(invalid("expected handshake packet"));
    }
    let protocol = read_varint(handshake, &mut cursor)?;
    let host_len = read_varint(handshake, &mut cursor)?;
    if !(0..=765).contains(&host_len) || cursor + host_len as usize + 3 > handshake.len() {
        return Err(invalid("invalid handshake host length"));
    }
    let host = std::str::from_utf8(&handshake[cursor..cursor + host_len as usize])
        .map_err(|_| invalid("handshake host is not valid UTF-8"))?;
    if host.encode_utf16().count() > 255 {
        return Err(invalid("handshake host exceeds 255 UTF-16 code units"));
    }
    cursor += host_len as usize + 2;
    let next_state = read_varint(handshake, &mut cursor)?;
    if cursor != handshake.len() {
        return Err(invalid("trailing handshake data"));
    }
    Ok((protocol, next_state))
}

struct ConnectionGuard;

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    }
}

fn serve(mut stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let handshake = read_frame(&mut stream)?;
    let (protocol, next_state) = parse_handshake(&handshake)?;
    match next_state {
        1 => status(&mut stream),
        2 => login(&mut stream, protocol),
        _ => Err(invalid("unsupported next state")),
    }
}

fn offline_bind_is_public(address: SocketAddr) -> bool {
    !address.ip().is_loopback()
}

fn public_offline_bind_is_refused(
    address: SocketAddr,
    offline_mode: bool,
    allow_public_offline: bool,
) -> bool {
    offline_mode && offline_bind_is_public(address) && !allow_public_offline
}

fn protocol_is_supported(protocol: i32) -> bool {
    protocol == PROTOCOL
}

fn login(stream: &mut TcpStream, protocol: i32) -> io::Result<()> {
    if !protocol_is_supported(protocol) {
        return send_disconnect(
            stream,
            "Unsupported protocol version; this server targets Minecraft Java Edition 26.3 (protocol 777)",
        );
    }
    let start = read_frame(stream)?;
    let (name, uuid) = match parse_login_start(&start) {
        Ok(parsed) => parsed,
        Err(_) => return send_disconnect(stream, "Invalid Login Start packet"),
    };
    if std::env::var(OFFLINE_MODE_ENV).as_deref() != Ok("1") {
        return send_disconnect(
            stream,
            "Offline login is disabled; set MC_OFFLINE_MODE=1 only on a trusted private network",
        );
    }
    eprintln!(
        "WARNING: offline mode does not authenticate players; usernames and UUIDs can be spoofed."
    );
    send_login_success(stream, name, uuid)?;
    let acknowledgement = read_frame(stream)?;
    if acknowledgement != [3] {
        return Err(invalid("expected Login Acknowledged packet"));
    }
    configuration(stream)?;
    play(stream, name)
}

fn send_login_success(stream: &mut TcpStream, name: &str, uuid: [u8; 16]) -> io::Result<()> {
    write_frame(stream, &login_success_packet(name, uuid)?)
}

fn login_success_packet(name: &str, uuid: [u8; 16]) -> io::Result<Vec<u8>> {
    let mut data = vec![2];
    data.extend_from_slice(&uuid);
    put_string(name, &mut data)?;
    put_varint(0, &mut data);
    data.extend_from_slice(&[0; 16]);
    Ok(data)
}

fn send_configuration_disconnect(stream: &mut TcpStream, reason: &str) -> io::Result<()> {
    let mut data = vec![2];
    data.extend_from_slice(&nbt::text_component(reason)?);
    write_frame(stream, &data)
}

struct Capture {
    before_known: Vec<Vec<u8>>,
    known: Vec<Vec<u8>>,
    play: Vec<Vec<u8>>,
}

impl Capture {
    fn load() -> io::Result<Self> {
        let directory = capture_dir();
        Ok(Self {
            before_known: read_capture_stream(&directory.join("config_before_known.bin"))?,
            known: read_capture_stream(&directory.join("config_known_packs.bin"))?,
            play: read_capture_stream(&directory.join("play_packets.bin"))?,
        })
    }
}

struct Registry {
    data: Vec<Vec<u8>>,
    tags: Vec<u8>,
}

impl Registry {
    fn load() -> io::Result<Self> {
        let directory = capture_dir();
        let tags = read_capture_stream(&directory.join("tags.bin"))?
            .into_iter()
            .next()
            .ok_or_else(|| invalid("empty tags capture"))?;
        Ok(Self {
            data: read_capture_stream(&directory.join("registry.bin"))?,
            tags,
        })
    }
}

fn capture_dir() -> PathBuf {
    std::env::var(CAPTURE_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("capture"))
}

fn capture() -> &'static Option<Capture> {
    CAPTURE.get_or_init(|| Capture::load().ok())
}

fn registry() -> &'static Option<Registry> {
    REGISTRY.get_or_init(|| Registry::load().ok())
}

fn read_capture_stream(path: &Path) -> io::Result<Vec<Vec<u8>>> {
    let bytes = std::fs::read(path)?;
    let mut cursor = 0;
    let mut packets = Vec::new();
    while cursor < bytes.len() {
        let length = read_varint(&bytes, &mut cursor)?;
        let length =
            usize::try_from(length).map_err(|_| invalid("negative capture packet length"))?;
        if length == 0
            || length > MAX_PACKET
            || cursor
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
        {
            return Err(invalid("invalid capture packet length"));
        }
        packets.push(bytes[cursor..cursor + length].to_vec());
        cursor += length;
    }
    Ok(packets)
}

fn send_packets(stream: &mut TcpStream, packets: &[Vec<u8>]) -> io::Result<()> {
    for packet in packets {
        write_frame(stream, packet)?;
    }
    Ok(())
}

fn configuration(stream: &mut TcpStream) -> io::Result<()> {
    let Some(capture) = capture() else {
        return send_configuration_disconnect(
            stream,
            "Configuration and gameplay are not implemented yet",
        );
    };
    let Some(registry) = registry() else {
        return send_configuration_disconnect(
            stream,
            "Configuration and gameplay are not implemented yet",
        );
    };
    send_packets(stream, &capture.before_known)?;
    send_packets(stream, &capture.known)?;
    loop {
        let packet = read_frame(stream)?;
        let mut cursor = 0;
        match read_varint(&packet, &mut cursor)? {
            0x07 => break,
            0x00 | 0x01 | 0x02 | 0x04 | 0x05 | 0x06 => continue,
            _ => {
                return Err(invalid(
                    "unexpected configuration packet before known packs",
                ));
            }
        }
    }
    send_packets(stream, &registry.data)?;
    write_frame(stream, &registry.tags)?;
    write_frame(stream, &[3])?;
    loop {
        let packet = read_frame(stream)?;
        let mut cursor = 0;
        match read_varint(&packet, &mut cursor)? {
            0x03 => break,
            0x04 | 0x05 => write_frame(stream, &packet)?,
            0x00 | 0x01 | 0x02 | 0x06 => continue,
            _ => return Err(invalid("unexpected configuration packet after finish")),
        }
    }
    Ok(())
}

struct PlayerGuard {
    name: String,
}

impl Drop for PlayerGuard {
    fn drop(&mut self) {
        ONLINE_PLAYERS.fetch_sub(1, Ordering::Relaxed);
        eprintln!("{} left the game", self.name);
    }
}

fn keep_alive_packet(id: i64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    put_varint(PLAY_KEEP_ALIVE_PACKET_ID, &mut data);
    data.extend_from_slice(&id.to_be_bytes());
    data
}

fn patch_login_entity_id(packet: &[u8], entity_id: i32) -> io::Result<Vec<u8>> {
    let mut cursor = 0;
    let packet_id = read_varint(packet, &mut cursor)?;
    if packet_id != 0x32 {
        return Ok(packet.to_vec());
    }
    let _old_entity_id = read_varint(packet, &mut cursor)?;
    let mut patched = Vec::with_capacity(packet.len() + 5);
    put_varint(packet_id, &mut patched);
    put_varint(entity_id, &mut patched);
    patched.extend_from_slice(&packet[cursor..]);
    Ok(patched)
}

fn play(stream: &mut TcpStream, name: &str) -> io::Result<()> {
    let Some(capture) = capture() else {
        return Ok(());
    };
    let entity_id = NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed);
    let mut first = true;
    for packet in &capture.play {
        if first {
            write_frame(stream, &patch_login_entity_id(packet, entity_id)?)?;
            first = false;
        } else {
            write_frame(stream, packet)?;
        }
    }
    ONLINE_PLAYERS.fetch_add(1, Ordering::Relaxed);
    eprintln!("{} joined the game (entity {entity_id})", name);
    let _player = PlayerGuard {
        name: name.to_owned(),
    };
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut last_keep_alive = Instant::now();
    let mut keep_alive_id = 0i64;
    loop {
        if last_keep_alive.elapsed() >= PLAY_KEEP_ALIVE_INTERVAL {
            write_frame(stream, &keep_alive_packet(keep_alive_id))?;
            keep_alive_id = keep_alive_id.wrapping_add(1);
            last_keep_alive = Instant::now();
        }
        match read_frame(stream) {
            Ok(_) => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                ) =>
            {
                return Ok(());
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
}

fn status_json() -> String {
    format!(
        r#"{{"version":{{"name":"26.3","protocol":777}},"players":{{"max":20,"online":{}}},"description":{{"text":"Minecraft Rust Server 26.3"}}}}"#,
        ONLINE_PLAYERS.load(Ordering::Relaxed)
    )
}

fn parse_login_start(packet: &[u8]) -> io::Result<(&str, [u8; 16])> {
    let mut cursor = 0;
    if read_varint(packet, &mut cursor)? != 0 {
        return Err(invalid("expected login start packet"));
    }
    let name_len = read_varint(packet, &mut cursor)?;
    if !(1..=16).contains(&name_len) || cursor + name_len as usize + 16 != packet.len() {
        return Err(invalid("invalid login name length or packet size"));
    }
    let name = std::str::from_utf8(&packet[cursor..cursor + name_len as usize])
        .map_err(|_| invalid("login name is not valid UTF-8"))?;
    if !is_valid_username(name) {
        return Err(invalid(
            "login name must be 1-16 ASCII letters, digits, or underscores",
        ));
    }
    cursor += name_len as usize;
    let uuid: [u8; 16] = packet[cursor..]
        .try_into()
        .map_err(|_| invalid("login start UUID must be 16 bytes"))?;
    Ok((name, uuid))
}

fn is_valid_username(name: &str) -> bool {
    (1..=16).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn status(stream: &mut TcpStream) -> io::Result<()> {
    let request = read_frame(stream)?;
    if request != [0] {
        return Err(invalid("expected status request"));
    }
    let mut response = vec![0];
    put_string(&status_json(), &mut response)?;
    write_frame(stream, &response)?;

    let ping = read_frame(stream)?;
    if ping.len() != 9 || ping[0] != 1 {
        return Err(invalid("expected status ping"));
    }
    write_frame(stream, &ping)
}

fn send_disconnect(stream: &mut TcpStream, reason: &str) -> io::Result<()> {
    let mut data = vec![0];
    put_string(&text_component(reason), &mut data)?;
    write_frame(stream, &data)
}

fn text_component(text: &str) -> String {
    format!("{{\"text\":\"{}\"}}", json_escape(text))
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            character if character < '\u{20}' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut first = [0];
    stream.read_exact(&mut first)?;
    let mut length = (first[0] & 0x7f) as usize;
    let mut shift = 7;
    let mut byte = first[0];
    let mut count = 1;
    while byte & 0x80 != 0 {
        if count == 3 {
            return Err(invalid("frame length VarInt too long"));
        }
        stream.read_exact(&mut first)?;
        byte = first[0];
        count += 1;
        length |= ((byte & 0x7f) as usize)
            .checked_shl(shift)
            .ok_or_else(|| invalid("frame length overflow"))?;
        shift += 7;
    }
    if length == 0 || length > MAX_PACKET {
        return Err(invalid("frame length outside allowed range"));
    }
    let mut packet = vec![0; length];
    stream.read_exact(&mut packet)?;
    Ok(packet)
}

fn write_frame(stream: &mut TcpStream, packet: &[u8]) -> io::Result<()> {
    if packet.len() > MAX_PACKET {
        return Err(invalid("outgoing packet exceeds limit"));
    }
    let mut frame = Vec::with_capacity(packet.len() + 3);
    put_varint(packet.len() as i32, &mut frame);
    frame.extend_from_slice(packet);
    stream.write_all(&frame)
}

fn read_varint(data: &[u8], cursor: &mut usize) -> io::Result<i32> {
    let mut value = 0u32;
    for index in 0..5 {
        let byte = *data
            .get(*cursor)
            .ok_or_else(|| invalid("truncated VarInt"))?;
        *cursor += 1;
        if index == 4 && byte > 0x0f {
            return Err(invalid("VarInt exceeds 32 bits"));
        }
        value |= ((byte & 0x7f) as u32) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(invalid("VarInt too long"))
}

fn put_varint(value: i32, out: &mut Vec<u8>) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            out.push(value as u8);
            return;
        }
        out.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

fn put_string(value: &str, out: &mut Vec<u8>) -> io::Result<()> {
    let len = i32::try_from(value.len()).map_err(|_| invalid("string too long"))?;
    put_varint(len, out);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[unsafe(no_mangle)]
pub extern "C" fn minecraft_rust_protocol_version() -> i32 {
    PROTOCOL
}

/// # Safety
/// `address` must be null or point to a valid NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn minecraft_rust_run(address: *const std::ffi::c_char) -> i32 {
    let address = if address.is_null() {
        "127.0.0.1:25565".to_owned()
    } else {
        match unsafe { std::ffi::CStr::from_ptr(address) }.to_str() {
            Ok(address) => address.to_owned(),
            Err(_) => return -1,
        }
    };
    match run(&address) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("server error: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip_signed_protocol_values() {
        for value in [0, 1, 127, 128, 777, i32::MAX, -1, i32::MIN] {
            let mut encoded = Vec::new();
            put_varint(value, &mut encoded);
            let mut cursor = 0;
            assert_eq!(read_varint(&encoded, &mut cursor).unwrap(), value);
            assert_eq!(cursor, encoded.len());
        }
    }

    #[test]
    fn rejects_varint_overflow_and_truncation() {
        let mut cursor = 0;
        assert!(read_varint(&[0xff, 0xff, 0xff, 0xff, 0x10], &mut cursor).is_err());
        cursor = 0;
        assert!(read_varint(&[0x80], &mut cursor).is_err());
    }

    fn handshake_bytes(host: &[u8]) -> Vec<u8> {
        let mut handshake = vec![0];
        put_varint(PROTOCOL, &mut handshake);
        put_varint(host.len() as i32, &mut handshake);
        handshake.extend_from_slice(host);
        handshake.extend_from_slice(&25565u16.to_be_bytes());
        put_varint(1, &mut handshake);
        handshake
    }

    #[test]
    fn parses_valid_multibyte_handshake_host() {
        let host = "例".repeat(255);
        let handshake = handshake_bytes(host.as_bytes());
        assert_eq!(parse_handshake(&handshake).unwrap(), (PROTOCOL, 1));
    }

    #[test]
    fn rejects_invalid_utf8_handshake_host() {
        let handshake = handshake_bytes(&[0xff]);
        assert!(parse_handshake(&handshake).is_err());
    }

    #[test]
    fn rejects_handshake_host_over_255_utf16_units() {
        let host = "😀".repeat(128);
        let handshake = handshake_bytes(host.as_bytes());
        assert!(parse_handshake(&handshake).is_err());
    }

    #[test]
    fn detects_public_offline_bind_addresses() {
        assert!(offline_bind_is_public("0.0.0.0:25565".parse().unwrap()));
        assert!(offline_bind_is_public("[::]:25565".parse().unwrap()));
        assert!(!offline_bind_is_public("127.0.0.1:25565".parse().unwrap()));
        assert!(!offline_bind_is_public("[::1]:25565".parse().unwrap()));
    }

    #[test]
    fn login_success_encodes_profile_and_session() {
        let uuid = [0x11; 16];
        let packet = login_success_packet("Alex", uuid).unwrap();
        assert_eq!(packet[0], 2);
        assert_eq!(&packet[1..17], &uuid);
        let mut cursor = 17;
        let name_len = read_varint(&packet, &mut cursor).unwrap() as usize;
        assert_eq!(&packet[cursor..cursor + name_len], b"Alex");
        cursor += name_len;
        assert_eq!(read_varint(&packet, &mut cursor).unwrap(), 0);
        assert_eq!(&packet[cursor..], &[0; 16]);
    }

    #[test]
    fn parses_login_start_name_and_uuid() {
        let mut packet = vec![0];
        put_varint(4, &mut packet);
        packet.extend_from_slice(b"Test");
        packet.extend(0u8..16);
        let (name, uuid) = parse_login_start(&packet).unwrap();
        assert_eq!(name, "Test");
        assert_eq!(uuid, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    }

    #[test]
    fn rejects_malformed_login_start() {
        assert!(parse_login_start(&[0, 0]).is_err());
        let mut packet = vec![0, 1, 0xff];
        packet.extend([0; 16]);
        assert!(parse_login_start(&packet).is_err());
    }

    #[test]
    fn rejects_login_names_outside_ascii_username_charset() {
        for name in ["Bad Name", "bad-name", "bad.name", "nama_é", "bad!"] {
            let mut packet = vec![0];
            put_varint(name.len() as i32, &mut packet);
            packet.extend_from_slice(name.as_bytes());
            packet.extend([0; 16]);
            assert!(
                parse_login_start(&packet).is_err(),
                "{name} should be rejected"
            );
        }
        let mut packet = vec![0];
        put_varint(1, &mut packet);
        packet.extend_from_slice(b"_");
        packet.extend([0; 16]);
        assert!(parse_login_start(&packet).is_ok());
    }

    #[test]
    fn protocol_check_accepts_only_777() {
        assert!(protocol_is_supported(777));
        assert!(!protocol_is_supported(776));
        assert!(!protocol_is_supported(778));
    }

    #[test]
    fn public_offline_bind_is_refused_without_override() {
        let public: SocketAddr = "0.0.0.0:25565".parse().unwrap();
        let loopback: SocketAddr = "127.0.0.1:25565".parse().unwrap();
        assert!(public_offline_bind_is_refused(public, true, false));
        assert!(!public_offline_bind_is_refused(public, true, true));
        assert!(!public_offline_bind_is_refused(public, false, false));
        assert!(!public_offline_bind_is_refused(loopback, true, false));
    }

    #[test]
    fn text_component_escapes_json_metacharacters() {
        assert_eq!(
            text_component("quote \" slash \\ newline\n tab\t nul\u{0}"),
            "{\"text\":\"quote \\\" slash \\\\ newline\\n tab\\t nul\\u0000\"}"
        );
        assert_eq!(text_component("é"), "{\"text\":\"é\"}");
    }

    #[test]
    fn keep_alive_packet_encodes_id() {
        let packet = keep_alive_packet(0x0102_0304_0506_0708);
        assert_eq!(packet[0], 0x2C);
        assert_eq!(&packet[1..], &0x0102_0304_0506_0708i64.to_be_bytes());
    }

    #[test]
    fn patches_play_login_entity_id() {
        let patched = patch_login_entity_id(&[0x32, 0, 0xAA, 0xBB], 300).unwrap();
        let mut cursor = 0;
        assert_eq!(read_varint(&patched, &mut cursor).unwrap(), 0x32);
        assert_eq!(read_varint(&patched, &mut cursor).unwrap(), 300);
        assert_eq!(&patched[cursor..], &[0xAA, 0xBB]);
    }

    #[test]
    fn status_json_reports_online_players() {
        let previous = ONLINE_PLAYERS.load(Ordering::Relaxed);
        ONLINE_PLAYERS.store(3, Ordering::Relaxed);
        assert!(status_json().contains("\"online\":3"));
        ONLINE_PLAYERS.store(previous, Ordering::Relaxed);
    }
}
