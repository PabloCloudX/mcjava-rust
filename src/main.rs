use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

const PROTOCOL: i32 = 777;
const MAX_PACKET: usize = 1_048_576;
const STATUS_JSON: &str = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":0,"online":0,"sample":[]},"description":{"text":"Minecraft Rust Server 26.3"}}"#;

fn main() -> io::Result<()> {
    let address = std::env::args().nth(1).unwrap_or_else(|| "0.0.0.0:25565".into());
    let listener = TcpListener::bind(&address)?;
    eprintln!("Minecraft Rust Server status listener on {address} (protocol {PROTOCOL})");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(error) = serve(stream) {
                        eprintln!("connection ended: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn serve(mut stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let handshake = read_frame(&mut stream)?;
    let mut cursor = 0;
    if read_varint(&handshake, &mut cursor)? != 0 {
        return Err(invalid("expected handshake packet"));
    }
    let protocol = read_varint(&handshake, &mut cursor)?;
    let host_len = read_varint(&handshake, &mut cursor)?;
    if !(0..=255).contains(&host_len) || cursor + host_len as usize + 3 > handshake.len() {
        return Err(invalid("invalid handshake host"));
    }
    cursor += host_len as usize;
    if cursor + 2 > handshake.len() {
        return Err(invalid("missing handshake port"));
    }
    cursor += 2;
    let next_state = read_varint(&handshake, &mut cursor)?;
    if cursor != handshake.len() {
        return Err(invalid("trailing handshake data"));
    }
    match next_state {
        1 => status(&mut stream),
        2 => {
            let _ = protocol;
            send_disconnect(&mut stream, "Login is not implemented yet")
        }
        _ => Err(invalid("unsupported next state")),
    }
}

fn status(stream: &mut TcpStream) -> io::Result<()> {
    let request = read_frame(stream)?;
    if request != [0] {
        return Err(invalid("expected status request"));
    }
    let mut response = vec![0];
    put_string(&mut response, STATUS_JSON)?;
    write_frame(stream, &response)?;

    let ping = read_frame(stream)?;
    if ping.len() != 9 || ping[0] != 1 {
        return Err(invalid("expected status ping"));
    }
    write_frame(stream, &ping)
}

fn send_disconnect(stream: &mut TcpStream, reason: &str) -> io::Result<()> {
    let mut data = vec![0];
    put_string(&mut data, &format!("{{\"text\":\"{reason}\"}}"))?;
    write_frame(stream, &data)
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut first = [0];
    stream.read_exact(&mut first)?;
    let mut prefix = vec![first[0]];
    let mut length = (first[0] & 0x7f) as usize;
    let mut shift = 7;
    while first[0] & 0x80 != 0 {
        if prefix.len() == 3 {
            return Err(invalid("frame length VarInt too long"));
        }
        stream.read_exact(&mut first)?;
        prefix.push(first[0]);
        length |= ((first[0] & 0x7f) as usize)
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
        let byte = *data.get(*cursor).ok_or_else(|| invalid("truncated VarInt"))?;
        *cursor += 1;
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
}
