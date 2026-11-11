mod nbt;

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const PROTOCOL: i32 = 777;
const MAX_CONNECTIONS: usize = 128;
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
const MAX_PACKET: usize = 1_048_576;
const STATUS_JSON: &str = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":0,"online":0,"sample":[]},"description":{"text":"Minecraft Rust Server 26.3"}}"#;

fn main() -> io::Result<()> {
    let _nbt_parser = nbt::parse;
    let _nbt_writer = nbt::write;
    let _nbt_integer_lookup: fn(&nbt::Compound, &str) -> Option<i32> = nbt::Compound::get_int;
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:25565".into());
    let offline_mode = std::env::var("MC_OFFLINE_MODE").as_deref() == Ok("1");
    let bind_addr: SocketAddr = address.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "listen address must be an IP:port socket address",
        )
    })?;
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
    eprintln!(
        "Minecraft Java Edition is a Mojang/Microsoft product. This server does not yet implement login or gameplay."
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
    let (_protocol, next_state) = parse_handshake(&handshake)?;
    match next_state {
        1 => status(&mut stream),
        2 => login(&mut stream),
        _ => Err(invalid("unsupported next state")),
    }
}

fn offline_bind_is_public(address: SocketAddr) -> bool {
    !address.ip().is_loopback()
}

fn login(stream: &mut TcpStream) -> io::Result<()> {
    let start = read_frame(stream)?;
    let (name, uuid) = parse_login_start(&start)?;
    if std::env::var("MC_OFFLINE_MODE").as_deref() != Ok("1") {
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
    send_configuration_disconnect(stream, "Configuration and gameplay are not implemented yet")
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
    put_string(&format!("{{\"text\":\"{reason}\"}}"), &mut data)?;
    write_frame(stream, &data)
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
    if name.chars().any(|character| character.is_control()) {
        return Err(invalid("login name contains control characters"));
    }
    cursor += name_len as usize;
    let uuid: [u8; 16] = packet[cursor..]
        .try_into()
        .map_err(|_| invalid("login start UUID must be 16 bytes"))?;
    Ok((name, uuid))
}

fn status(stream: &mut TcpStream) -> io::Result<()> {
    let request = read_frame(stream)?;
    if request != [0] {
        return Err(invalid("expected status request"));
    }
    let mut response = vec![0];
    put_string(STATUS_JSON, &mut response)?;
    write_frame(stream, &response)?;

    let ping = read_frame(stream)?;
    if ping.len() != 9 || ping[0] != 1 {
        return Err(invalid("expected status ping"));
    }
    write_frame(stream, &ping)
}

fn send_disconnect(stream: &mut TcpStream, reason: &str) -> io::Result<()> {
    let mut data = vec![0];
    put_string(&format!("{{\"text\":\"{reason}\"}}"), &mut data)?;
    write_frame(stream, &data)
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
}
