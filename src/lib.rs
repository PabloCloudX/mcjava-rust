mod nbt;

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
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
const PLAY_KEEP_ALIVE_PACKET_ID: i32 = 0x2D;
const SERVERBOUND_KEEP_ALIVE_PACKET_ID: i32 = 0x1C;
const ADD_ENTITY_PACKET_ID: i32 = 0x01;
const MOVE_ENTITY_POS_ROT_PACKET_ID: i32 = 0x37;
const MOVE_ENTITY_ROT_PACKET_ID: i32 = 0x39;
const PLAYER_INFO_REMOVE_PACKET_ID: i32 = 0x46;
const PLAYER_INFO_UPDATE_PACKET_ID: i32 = 0x47;
const REMOVE_ENTITIES_PACKET_ID: i32 = 0x4E;
const SET_CHUNK_CACHE_CENTER_PACKET_ID: i32 = 0x60;
const LEVEL_CHUNK_WITH_LIGHT_PACKET_ID: i32 = 0x2E;
const PLAYER_ENTITY_TYPE_ID: i32 = 159;
const VIEW_DISTANCE: i32 = 2;
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
    play(stream, name, uuid)
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
    chunks: Vec<(i32, i32, Vec<u8>)>,
}

impl Capture {
    fn load() -> io::Result<Self> {
        let directory = capture_dir();
        let play = read_capture_stream(&directory.join("play_packets.bin"))?;
        let chunks = play
            .iter()
            .filter_map(|packet| chunk_coords(packet).map(|(x, z)| (x, z, packet.clone())))
            .collect();
        Ok(Self {
            before_known: read_capture_stream(&directory.join("config_before_known.bin"))?,
            known: read_capture_stream(&directory.join("config_known_packs.bin"))?,
            play,
            chunks,
        })
    }
}

fn chunk_coords(packet: &[u8]) -> Option<(i32, i32)> {
    let mut cursor = 0;
    if read_varint(packet, &mut cursor).ok()? != LEVEL_CHUNK_WITH_LIGHT_PACKET_ID
        || cursor + 8 > packet.len()
    {
        return None;
    }
    let x = i32::from_be_bytes(packet[cursor..cursor + 4].try_into().ok()?);
    let z = i32::from_be_bytes(packet[cursor + 4..cursor + 8].try_into().ok()?);
    Some((x, z))
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

struct Player {
    entity_id: i32,
    uuid: [u8; 16],
    name: String,
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
    writer: Arc<Mutex<TcpStream>>,
    seen_chunks: HashSet<(i32, i32)>,
}

static PLAYERS: OnceLock<Mutex<HashMap<i32, Arc<Mutex<Player>>>>> = OnceLock::new();

fn players() -> &'static Mutex<HashMap<i32, Arc<Mutex<Player>>>> {
    PLAYERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn send_shared(writer: &Arc<Mutex<TcpStream>>, packet: &[u8]) -> io::Result<()> {
    let mut stream = writer
        .lock()
        .map_err(|_| invalid("player writer lock poisoned"))?;
    write_frame(&mut stream, packet)
}

fn send_to_all(packets: &[Vec<u8>]) {
    let writers: Vec<Arc<Mutex<TcpStream>>> = {
        let Ok(map) = players().lock() else {
            return;
        };
        map.values()
            .filter_map(|player| player.lock().ok().map(|player| Arc::clone(&player.writer)))
            .collect()
    };
    for writer in writers {
        for packet in packets {
            let _ = send_shared(&writer, packet);
        }
    }
}

fn broadcast_to_others(except_entity_id: i32, packets: &[Vec<u8>]) {
    let writers: Vec<Arc<Mutex<TcpStream>>> = {
        let Ok(map) = players().lock() else {
            return;
        };
        map.values()
            .filter_map(|player| {
                let player = player.lock().ok()?;
                if player.entity_id == except_entity_id {
                    None
                } else {
                    Some(Arc::clone(&player.writer))
                }
            })
            .collect()
    };
    for writer in writers {
        for packet in packets {
            let _ = send_shared(&writer, packet);
        }
    }
}

fn angle_byte(angle: f32) -> u8 {
    ((angle * 256.0 / 360.0) as i32) as u8
}

fn player_info_packets(uuid: &[u8; 16], name: &str) -> io::Result<Vec<Vec<u8>>> {
    let mut add = vec![PLAYER_INFO_UPDATE_PACKET_ID as u8];
    put_varint(0x01, &mut add);
    put_varint(1, &mut add);
    add.extend_from_slice(uuid);
    put_string(name, &mut add)?;
    put_varint(0, &mut add);

    let mut game_mode = vec![PLAYER_INFO_UPDATE_PACKET_ID as u8];
    put_varint(0x04, &mut game_mode);
    put_varint(1, &mut game_mode);
    game_mode.extend_from_slice(uuid);
    put_varint(0, &mut game_mode);

    let mut listed = vec![PLAYER_INFO_UPDATE_PACKET_ID as u8];
    put_varint(0x08, &mut listed);
    put_varint(1, &mut listed);
    listed.extend_from_slice(uuid);
    listed.push(1);

    let mut latency = vec![PLAYER_INFO_UPDATE_PACKET_ID as u8];
    put_varint(0x10, &mut latency);
    put_varint(1, &mut latency);
    latency.extend_from_slice(uuid);
    put_varint(0, &mut latency);

    Ok(vec![add, game_mode, listed, latency])
}

fn player_info_remove_packet(uuid: &[u8; 16]) -> Vec<u8> {
    let mut data = vec![PLAYER_INFO_REMOVE_PACKET_ID as u8];
    put_varint(1, &mut data);
    data.extend_from_slice(uuid);
    data
}

fn remove_entities_packet(entity_id: i32) -> Vec<u8> {
    let mut data = vec![REMOVE_ENTITIES_PACKET_ID as u8];
    put_varint(1, &mut data);
    put_varint(entity_id, &mut data);
    data
}

fn add_entity_packet(player: &Player) -> Vec<u8> {
    let mut data = vec![ADD_ENTITY_PACKET_ID as u8];
    put_varint(player.entity_id, &mut data);
    data.extend_from_slice(&player.uuid);
    put_varint(PLAYER_ENTITY_TYPE_ID, &mut data);
    data.extend_from_slice(&player.x.to_be_bytes());
    data.extend_from_slice(&player.y.to_be_bytes());
    data.extend_from_slice(&player.z.to_be_bytes());
    data.extend_from_slice(&[0; 6]);
    data.push(angle_byte(player.pitch));
    data.push(angle_byte(player.yaw));
    data.push(angle_byte(player.yaw));
    put_varint(0, &mut data);
    data
}

fn delta_short(delta: f64) -> i16 {
    (delta * 4096.0)
        .round()
        .clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

fn move_entity_pos_rot_packet(
    entity_id: i32,
    dx: f64,
    dy: f64,
    dz: f64,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
) -> Vec<u8> {
    let mut data = vec![MOVE_ENTITY_POS_ROT_PACKET_ID as u8];
    put_varint(entity_id, &mut data);
    put_varint(2 | i32::from(on_ground), &mut data);
    data.extend_from_slice(&delta_short(dx).to_be_bytes());
    data.extend_from_slice(&delta_short(dy).to_be_bytes());
    data.extend_from_slice(&delta_short(dz).to_be_bytes());
    data.push(angle_byte(yaw));
    data.push(angle_byte(pitch));
    data
}

fn move_entity_rot_packet(entity_id: i32, yaw: f32, pitch: f32, on_ground: bool) -> Vec<u8> {
    let mut data = vec![MOVE_ENTITY_ROT_PACKET_ID as u8];
    put_varint(entity_id, &mut data);
    data.push(u8::from(on_ground));
    data.push(angle_byte(yaw));
    data.push(angle_byte(pitch));
    data
}

fn set_chunk_cache_center_packet(x: i32, z: i32) -> Vec<u8> {
    let mut data = vec![SET_CHUNK_CACHE_CENTER_PACKET_ID as u8];
    put_varint(x, &mut data);
    put_varint(z, &mut data);
    data
}

fn chunk_of(value: f64) -> i32 {
    (value / 16.0).floor() as i32
}

fn read_f64(data: &[u8], cursor: &mut usize) -> io::Result<f64> {
    let end = cursor
        .checked_add(8)
        .ok_or_else(|| invalid("double cursor overflow"))?;
    let bytes = data
        .get(*cursor..end)
        .ok_or_else(|| invalid("truncated double"))?;
    *cursor = end;
    Ok(f64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| invalid("invalid double width"))?,
    ))
}

fn read_f32(data: &[u8], cursor: &mut usize) -> io::Result<f32> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| invalid("float cursor overflow"))?;
    let bytes = data
        .get(*cursor..end)
        .ok_or_else(|| invalid("truncated float"))?;
    *cursor = end;
    Ok(f32::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| invalid("invalid float width"))?,
    ))
}

fn parse_player_position(packet: &[u8]) -> io::Result<(f64, f64, f64, f32, f32)> {
    let mut cursor = 0;
    if read_varint(packet, &mut cursor)? != 0x49 {
        return Err(invalid("expected player position packet"));
    }
    let _teleport_id = read_varint(packet, &mut cursor)?;
    let x = read_f64(packet, &mut cursor)?;
    let y = read_f64(packet, &mut cursor)?;
    let z = read_f64(packet, &mut cursor)?;
    let _delta_x = read_f64(packet, &mut cursor)?;
    let _delta_y = read_f64(packet, &mut cursor)?;
    let _delta_z = read_f64(packet, &mut cursor)?;
    let yaw = read_f32(packet, &mut cursor)?;
    let pitch = read_f32(packet, &mut cursor)?;
    Ok((x, y, z, yaw, pitch))
}

fn patch_play_packet(packet: &[u8], entity_id: i32) -> io::Result<Vec<u8>> {
    let mut cursor = 0;
    let packet_id = read_varint(packet, &mut cursor)?;
    match packet_id {
        0x32 => {
            if cursor + 4 > packet.len() {
                return Err(invalid("truncated Play Login entity ID"));
            }
            let mut patched = Vec::with_capacity(packet.len());
            patched.extend_from_slice(&packet[..cursor]);
            patched.extend_from_slice(&entity_id.to_be_bytes());
            patched.extend_from_slice(&packet[cursor + 4..]);
            Ok(patched)
        }
        0x22 => {
            if cursor + 4 > packet.len() {
                return Err(invalid("truncated entity event entity ID"));
            }
            let mut patched = packet.to_vec();
            patched[cursor..cursor + 4].copy_from_slice(&entity_id.to_be_bytes());
            Ok(patched)
        }
        0x65 | 0x86 => {
            let mut inner = cursor;
            let _old_entity_id = read_varint(packet, &mut inner)?;
            let mut patched = Vec::with_capacity(packet.len() + 2);
            patched.extend_from_slice(&packet[..cursor]);
            put_varint(entity_id, &mut patched);
            patched.extend_from_slice(&packet[inner..]);
            Ok(patched)
        }
        _ => Ok(packet.to_vec()),
    }
}

struct Movement {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    on_ground: bool,
}

fn parse_movement(
    packet_id: i32,
    packet: &[u8],
    mut cursor: usize,
) -> io::Result<Option<Movement>> {
    let mut movement = Movement {
        x: None,
        y: None,
        z: None,
        yaw: None,
        pitch: None,
        on_ground: false,
    };
    match packet_id {
        0x1e => {
            movement.x = Some(read_f64(packet, &mut cursor)?);
            movement.y = Some(read_f64(packet, &mut cursor)?);
            movement.z = Some(read_f64(packet, &mut cursor)?);
        }
        0x1f => {
            movement.x = Some(read_f64(packet, &mut cursor)?);
            movement.y = Some(read_f64(packet, &mut cursor)?);
            movement.z = Some(read_f64(packet, &mut cursor)?);
            movement.yaw = Some(read_f32(packet, &mut cursor)?);
            movement.pitch = Some(read_f32(packet, &mut cursor)?);
        }
        0x20 => {
            movement.yaw = Some(read_f32(packet, &mut cursor)?);
            movement.pitch = Some(read_f32(packet, &mut cursor)?);
        }
        0x21 => {}
        _ => return Ok(None),
    }
    let flags = *packet
        .get(cursor)
        .ok_or_else(|| invalid("truncated movement flags"))?;
    movement.on_ground = flags & 1 != 0;
    Ok(Some(movement))
}

fn update_player_movement(player: &Arc<Mutex<Player>>, movement: Movement) {
    let (
        entity_id,
        writer,
        moved,
        old_chunk,
        new_chunk,
        dx,
        dy,
        dz,
        yaw,
        pitch,
        on_ground,
        chunk_packets,
    ) = {
        let Ok(mut player) = player.lock() else {
            return;
        };
        let old_x = player.x;
        let old_y = player.y;
        let old_z = player.z;
        let old_yaw = player.yaw;
        let old_pitch = player.pitch;
        let new_x = movement.x.unwrap_or(old_x);
        let new_y = movement.y.unwrap_or(old_y);
        let new_z = movement.z.unwrap_or(old_z);
        let new_yaw = movement.yaw.unwrap_or(old_yaw);
        let new_pitch = movement.pitch.unwrap_or(old_pitch);
        let moved = (new_x - old_x).abs() > f64::EPSILON
            || (new_y - old_y).abs() > f64::EPSILON
            || (new_z - old_z).abs() > f64::EPSILON
            || (new_yaw - old_yaw).abs() > f32::EPSILON
            || (new_pitch - old_pitch).abs() > f32::EPSILON;
        if !moved {
            return;
        }
        player.x = new_x;
        player.y = new_y;
        player.z = new_z;
        player.yaw = new_yaw;
        player.pitch = new_pitch;
        let old_chunk = (chunk_of(old_x), chunk_of(old_z));
        let new_chunk = (chunk_of(new_x), chunk_of(new_z));
        let mut chunk_packets = Vec::new();
        if new_chunk != old_chunk
            && let Some(capture) = capture()
        {
            for (cx, cz, packet) in &capture.chunks {
                if (cx - new_chunk.0).abs() <= VIEW_DISTANCE
                    && (cz - new_chunk.1).abs() <= VIEW_DISTANCE
                    && player.seen_chunks.insert((*cx, *cz))
                {
                    chunk_packets.push(packet.clone());
                }
            }
        }
        (
            player.entity_id,
            Arc::clone(&player.writer),
            moved,
            old_chunk,
            new_chunk,
            new_x - old_x,
            new_y - old_y,
            new_z - old_z,
            new_yaw,
            new_pitch,
            movement.on_ground,
            chunk_packets,
        )
    };
    if !moved {
        return;
    }
    if dx.abs() <= 8.0 && dy.abs() <= 8.0 && dz.abs() <= 8.0 {
        let packet = move_entity_pos_rot_packet(entity_id, dx, dy, dz, yaw, pitch, on_ground);
        broadcast_to_others(entity_id, &[packet]);
    } else if (yaw - 0.0).abs() > f32::EPSILON {
        let packet = move_entity_rot_packet(entity_id, yaw, pitch, on_ground);
        broadcast_to_others(entity_id, &[packet]);
    }
    if new_chunk != old_chunk {
        let _ = send_shared(
            &writer,
            &set_chunk_cache_center_packet(new_chunk.0, new_chunk.1),
        );
        for packet in chunk_packets {
            let _ = send_shared(&writer, &packet);
        }
    }
}

fn register_player(player: Arc<Mutex<Player>>) -> io::Result<()> {
    let (entity_id, uuid, name, writer, add_self) = {
        let player = player
            .lock()
            .map_err(|_| invalid("player state lock poisoned"))?;
        (
            player.entity_id,
            player.uuid,
            player.name.clone(),
            Arc::clone(&player.writer),
            add_entity_packet(&player),
        )
    };
    let existing: Vec<Arc<Mutex<Player>>> = {
        let map = players()
            .lock()
            .map_err(|_| invalid("player map lock poisoned"))?;
        map.values().cloned().collect()
    };
    {
        let mut map = players()
            .lock()
            .map_err(|_| invalid("player map lock poisoned"))?;
        map.insert(entity_id, Arc::clone(&player));
    }
    let self_info = player_info_packets(&uuid, &name)?;
    send_to_all(&self_info);
    broadcast_to_others(entity_id, &[add_self]);
    for other in existing {
        let (other_info, add_other) = {
            let other = other
                .lock()
                .map_err(|_| invalid("player state lock poisoned"))?;
            (
                player_info_packets(&other.uuid, &other.name)?,
                add_entity_packet(&other),
            )
        };
        for packet in &other_info {
            send_shared(&writer, packet)?;
        }
        send_shared(&writer, &add_other)?;
    }
    Ok(())
}

struct PlayerGuard {
    entity_id: i32,
    uuid: [u8; 16],
    name: String,
}

impl Drop for PlayerGuard {
    fn drop(&mut self) {
        ONLINE_PLAYERS.fetch_sub(1, Ordering::Relaxed);
        eprintln!("{} left the game", self.name);
        let removed = {
            let Ok(mut map) = players().lock() else {
                return;
            };
            map.remove(&self.entity_id)
        };
        if removed.is_some() {
            let packets = [
                player_info_remove_packet(&self.uuid),
                remove_entities_packet(self.entity_id),
            ];
            broadcast_to_others(self.entity_id, &packets);
        }
    }
}

fn keep_alive_packet(id: i64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    put_varint(PLAY_KEEP_ALIVE_PACKET_ID, &mut data);
    data.extend_from_slice(&id.to_be_bytes());
    data
}

fn play(stream: &mut TcpStream, name: &str, uuid: [u8; 16]) -> io::Result<()> {
    let Some(capture) = capture() else {
        return Ok(());
    };
    let entity_id = NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed);
    let mut seen_chunks = HashSet::new();
    let mut position: (f64, f64, f64, f32, f32) = (0.5, -60.0, 0.5, 0.0, 0.0);
    for packet in &capture.play {
        let mut cursor = 0;
        let packet_id = read_varint(packet, &mut cursor)?;
        if packet_id == PLAYER_INFO_UPDATE_PACKET_ID {
            continue;
        }
        if packet_id == 0x49
            && let Ok(parsed) = parse_player_position(packet)
        {
            position = parsed;
        }
        let patched = patch_play_packet(packet, entity_id)?;
        if let Some(coords) = chunk_coords(&patched) {
            seen_chunks.insert(coords);
        }
        write_frame(stream, &patched)?;
    }
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    let player = Arc::new(Mutex::new(Player {
        entity_id,
        uuid,
        name: name.to_owned(),
        x: position.0,
        y: position.1,
        z: position.2,
        yaw: position.3,
        pitch: position.4,
        writer: Arc::clone(&writer),
        seen_chunks,
    }));
    register_player(Arc::clone(&player))?;
    ONLINE_PLAYERS.fetch_add(1, Ordering::Relaxed);
    eprintln!("{} joined the game (entity {entity_id})", name);
    let _player = PlayerGuard {
        entity_id,
        uuid,
        name: name.to_owned(),
    };
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut last_keep_alive = Instant::now();
    let mut keep_alive_id = 0i64;
    loop {
        if last_keep_alive.elapsed() >= PLAY_KEEP_ALIVE_INTERVAL {
            send_shared(&writer, &keep_alive_packet(keep_alive_id))?;
            keep_alive_id = keep_alive_id.wrapping_add(1);
            last_keep_alive = Instant::now();
        }
        match read_frame(stream) {
            Ok(packet) => {
                let mut cursor = 0;
                let packet_id = read_varint(&packet, &mut cursor)?;
                if packet_id == SERVERBOUND_KEEP_ALIVE_PACKET_ID {
                    continue;
                }
                if let Some(movement) = parse_movement(packet_id, &packet, cursor)? {
                    update_player_movement(&player, movement);
                }
                continue;
            }
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
        assert_eq!(packet[0], 0x2D);
        assert_eq!(&packet[1..], &0x0102_0304_0506_0708i64.to_be_bytes());
    }

    #[test]
    fn patches_play_login_entity_id() {
        let original = [0x32, 0, 0, 0, 1, 0xAA, 0xBB];
        let patched = patch_play_packet(&original, 300).unwrap();
        assert_eq!(&patched[..1], &[0x32]);
        assert_eq!(&patched[1..5], &300i32.to_be_bytes());
        assert_eq!(&patched[5..], &[0xAA, 0xBB]);
    }

    #[test]
    fn status_json_reports_online_players() {
        let previous = ONLINE_PLAYERS.load(Ordering::Relaxed);
        ONLINE_PLAYERS.store(3, Ordering::Relaxed);
        assert!(status_json().contains("\"online\":3"));
        ONLINE_PLAYERS.store(previous, Ordering::Relaxed);
    }

    #[test]
    fn chunk_of_negative_positions_uses_floor() {
        assert_eq!(chunk_of(-0.5), -1);
        assert_eq!(chunk_of(0.5), 0);
        assert_eq!(chunk_of(-16.0), -1);
        assert_eq!(chunk_of(-16.1), -2);
        assert_eq!(chunk_of(15.9), 0);
        assert_eq!(chunk_of(16.0), 1);
    }

    #[test]
    fn patches_other_entity_id_references() {
        let entity_event = patch_play_packet(&[0x22, 0, 0, 0, 1, 0x18], 7).unwrap();
        assert_eq!(entity_event, [0x22, 0, 0, 0, 7, 0x18]);
        let entity_data = patch_play_packet(&[0x65, 1, 0x09, 0x03], 7).unwrap();
        assert_eq!(entity_data, [0x65, 7, 0x09, 0x03]);
        let attributes = patch_play_packet(&[0x86, 0x01, 0x03], 7).unwrap();
        assert_eq!(attributes, [0x86, 0x01, 7]);
    }

    #[test]
    fn move_entity_packet_encodes_fixed_point_delta() {
        let packet = move_entity_pos_rot_packet(3, 1.0, -0.5, 0.0, 90.0, 0.0, true);
        assert_eq!(packet[0], 0x37);
        let mut cursor = 1;
        assert_eq!(read_varint(&packet, &mut cursor).unwrap(), 3);
        assert_eq!(read_varint(&packet, &mut cursor).unwrap(), 3);
        let dx = i16::from_be_bytes(packet[cursor..cursor + 2].try_into().unwrap());
        let dy = i16::from_be_bytes(packet[cursor + 2..cursor + 4].try_into().unwrap());
        assert_eq!(dx, 4096);
        assert_eq!(dy, -2048);
        assert_eq!(&packet[cursor + 6..], &[64, 0]);
    }

    #[test]
    fn add_entity_packet_uses_player_type_id() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let writer = TcpStream::connect(address).unwrap();
        let _accepted = listener.accept().unwrap();
        let player = Player {
            entity_id: 5,
            uuid: [7; 16],
            name: "Alice".to_owned(),
            x: 1.5,
            y: -60.0,
            z: 2.5,
            yaw: 90.0,
            pitch: 0.0,
            writer: Arc::new(Mutex::new(writer)),
            seen_chunks: HashSet::new(),
        };
        let packet = add_entity_packet(&player);
        assert_eq!(packet[0], 0x01);
        let mut cursor = 1;
        assert_eq!(read_varint(&packet, &mut cursor).unwrap(), 5);
        assert_eq!(&packet[cursor..cursor + 16], &[7; 16]);
        cursor += 16;
        assert_eq!(
            read_varint(&packet, &mut cursor).unwrap(),
            PLAYER_ENTITY_TYPE_ID
        );
    }

    #[test]
    fn player_info_packets_include_name_and_uuid() {
        let uuid = [9; 16];
        let packets = player_info_packets(&uuid, "Alice").unwrap();
        assert_eq!(packets.len(), 4);
        assert_eq!(packets[0][0], 0x47);
        let mut cursor = 1;
        assert_eq!(read_varint(&packets[0], &mut cursor).unwrap(), 0x01);
        assert_eq!(read_varint(&packets[0], &mut cursor).unwrap(), 1);
        assert_eq!(&packets[0][cursor..cursor + 16], &uuid);
        cursor += 16;
        let name_len = read_varint(&packets[0], &mut cursor).unwrap() as usize;
        assert_eq!(&packets[0][cursor..cursor + name_len], b"Alice");
    }
}
