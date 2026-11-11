use std::io;

const MAX_DEPTH: usize = 64;
const MAX_ELEMENTS: usize = 1_000_000;

#[derive(Debug, PartialEq)]
pub struct Compound(pub Vec<(String, Tag)>);

impl Compound {
    pub fn get_int(&self, name: &str) -> Option<i32> {
        self.0
            .iter()
            .find_map(|(key, tag)| match (key.as_str(), tag) {
                (key, Tag::Int(value)) if key == name => Some(*value),
                _ => None,
            })
    }
}

#[derive(Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(u8, Vec<Tag>),
    Compound(Compound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

pub fn parse(input: &[u8]) -> io::Result<Compound> {
    let mut cursor = Cursor {
        bytes: input,
        pos: 0,
        elements: 0,
    };
    if cursor.byte()? != 10 {
        return Err(invalid("NBT root tag is not a compound"));
    }
    let _root_name = cursor.string()?;
    let root = cursor.compound(0)?;
    if cursor.pos != input.len() {
        return Err(invalid("trailing bytes after NBT root compound"));
    }
    Ok(root)
}

pub fn write(root: &Compound) -> io::Result<Vec<u8>> {
    let mut out = vec![10];
    put_string("", &mut out)?;
    write_compound(root, 0, &mut out)?;
    Ok(out)
}

fn write_compound(compound: &Compound, depth: usize, out: &mut Vec<u8>) -> io::Result<()> {
    if depth >= MAX_DEPTH {
        return Err(invalid("NBT nesting depth limit exceeded"));
    }
    for (name, tag) in &compound.0 {
        let kind = tag_kind(tag);
        out.push(kind);
        put_string(name, out)?;
        write_payload(tag, depth + 1, out)?;
        if out.len() > MAX_ELEMENTS * 16 {
            return Err(invalid("serialized NBT byte limit exceeded"));
        }
    }
    out.push(0);
    Ok(())
}

fn tag_kind(tag: &Tag) -> u8 {
    match tag {
        Tag::Byte(_) => 1,
        Tag::Short(_) => 2,
        Tag::Int(_) => 3,
        Tag::Long(_) => 4,
        Tag::Float(_) => 5,
        Tag::Double(_) => 6,
        Tag::ByteArray(_) => 7,
        Tag::String(_) => 8,
        Tag::List(_, _) => 9,
        Tag::Compound(_) => 10,
        Tag::IntArray(_) => 11,
        Tag::LongArray(_) => 12,
    }
}

fn write_payload(tag: &Tag, depth: usize, out: &mut Vec<u8>) -> io::Result<()> {
    if depth >= MAX_DEPTH {
        return Err(invalid("NBT nesting depth limit exceeded"));
    }
    match tag {
        Tag::Byte(value) => out.push(*value as u8),
        Tag::Short(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Int(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Long(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Float(value) => out.extend_from_slice(&value.to_bits().to_be_bytes()),
        Tag::Double(value) => out.extend_from_slice(&value.to_bits().to_be_bytes()),
        Tag::ByteArray(values) => {
            put_length(values.len(), out)?;
            out.extend(values.iter().map(|value| *value as u8));
        }
        Tag::String(value) => put_string(value, out)?,
        Tag::List(kind, values) => {
            if *kind > 12 || values.len() > MAX_ELEMENTS || (*kind == 0 && !values.is_empty()) {
                return Err(invalid("invalid NBT list kind or length"));
            }
            if values.iter().any(|value| tag_kind(value) != *kind) {
                return Err(invalid("NBT list contains mixed tag types"));
            }
            out.push(*kind);
            put_length(values.len(), out)?;
            for value in values {
                write_payload(value, depth + 1, out)?;
            }
        }
        Tag::Compound(compound) => write_compound(compound, depth, out)?,
        Tag::IntArray(values) => {
            put_length(values.len(), out)?;
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        Tag::LongArray(values) => {
            put_length(values.len(), out)?;
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
    Ok(())
}

fn put_length(len: usize, out: &mut Vec<u8>) -> io::Result<()> {
    if len > MAX_ELEMENTS {
        return Err(invalid("NBT array or list length outside allowed range"));
    }
    out.extend_from_slice(&(len as i32).to_be_bytes());
    Ok(())
}

fn put_string(value: &str, out: &mut Vec<u8>) -> io::Result<()> {
    let encoded = encode_modified_utf8(value)?;
    let len = u16::try_from(encoded.len()).map_err(|_| invalid("NBT string exceeds 65535 bytes"))?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&encoded);
    Ok(())
}

fn encode_modified_utf8(value: &str) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(value.len());
    for unit in value.encode_utf16() {
        match unit {
            0 => out.extend_from_slice(&[0xc0, 0x80]),
            1..=0x7f => out.push(unit as u8),
            0x80..=0x7ff => {
                out.push((0xc0 | (unit >> 6)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
            _ => {
                out.push((0xe0 | (unit >> 12)) as u8);
                out.push((0x80 | ((unit >> 6) & 0x3f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
        }
    }
    Ok(out)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
    elements: usize,
}

impl Cursor<'_> {
    fn take(&mut self, count: usize) -> io::Result<&[u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or_else(|| invalid("NBT length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated NBT"))?;
        self.pos = end;
        Ok(bytes)
    }

    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i64(&mut self) -> io::Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn length(&mut self) -> io::Result<usize> {
        let len = self.i32()?;
        if !(0..=MAX_ELEMENTS as i32).contains(&len) {
            return Err(invalid("NBT array or list length outside allowed range"));
        }
        self.elements = self
            .elements
            .checked_add(len as usize)
            .ok_or_else(|| invalid("NBT element count overflow"))?;
        if self.elements > MAX_ELEMENTS {
            return Err(invalid("NBT total element limit exceeded"));
        }
        Ok(len as usize)
    }

    fn string(&mut self) -> io::Result<String> {
        let len = self.u16()? as usize;
        let data = self.take(len)?;
        modified_utf8(data)
    }

    fn compound(&mut self, depth: usize) -> io::Result<Compound> {
        if depth >= MAX_DEPTH {
            return Err(invalid("NBT nesting depth limit exceeded"));
        }
        let mut tags = Vec::new();
        loop {
            let kind = self.byte()?;
            if kind == 0 {
                break;
            }
            self.elements = self
                .elements
                .checked_add(1)
                .ok_or_else(|| invalid("NBT element count overflow"))?;
            if self.elements > MAX_ELEMENTS {
                return Err(invalid("NBT total element limit exceeded"));
            }
            let name = self.string()?;
            tags.push((name, self.payload(kind, depth + 1)?));
        }
        Ok(Compound(tags))
    }

    fn payload(&mut self, kind: u8, depth: usize) -> io::Result<Tag> {
        if depth >= MAX_DEPTH {
            return Err(invalid("NBT nesting depth limit exceeded"));
        }
        match kind {
            1 => Ok(Tag::Byte(self.byte()? as i8)),
            2 => Ok(Tag::Short(self.u16()? as i16)),
            3 => Ok(Tag::Int(self.i32()?)),
            4 => Ok(Tag::Long(self.i64()?)),
            5 => Ok(Tag::Float(f32::from_bits(self.i32()? as u32))),
            6 => Ok(Tag::Double(f64::from_bits(self.i64()? as u64))),
            7 => {
                let len = self.length()?;
                Ok(Tag::ByteArray(
                    self.take(len)?.iter().map(|byte| *byte as i8).collect(),
                ))
            }
            8 => Ok(Tag::String(self.string()?)),
            9 => {
                let element_kind = self.byte()?;
                if element_kind > 12 {
                    return Err(invalid("unknown NBT list element type"));
                }
                let len = self.length()?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(self.payload(element_kind, depth + 1)?);
                }
                Ok(Tag::List(element_kind, values))
            }
            10 => Ok(Tag::Compound(self.compound(depth)?)),
            11 => {
                let len = self.length()?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(self.i32()?);
                }
                Ok(Tag::IntArray(values))
            }
            12 => {
                let len = self.length()?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(self.i64()?);
                }
                Ok(Tag::LongArray(values))
            }
            _ => Err(invalid("unknown NBT tag type")),
        }
    }
}

fn modified_utf8(data: &[u8]) -> io::Result<String> {
    let mut units = Vec::with_capacity(data.len());
    let mut pos = 0;
    while pos < data.len() {
        let first = data[pos];
        pos += 1;
        match first {
            1..=0x7f => units.push(first as u16),
            0xc0..=0xdf => {
                let second = *data
                    .get(pos)
                    .ok_or_else(|| invalid("truncated modified UTF-8"))?;
                pos += 1;
                if second & 0xc0 != 0x80 {
                    return Err(invalid("invalid modified UTF-8 continuation byte"));
                }
                let value = (((first & 0x1f) as u16) << 6) | (second & 0x3f) as u16;
                if value != 0 && value < 0x80 {
                    return Err(invalid("overlong modified UTF-8 sequence"));
                }
                if value == 0 && (first != 0xc0 || second != 0x80) {
                    return Err(invalid("invalid modified UTF-8 NUL"));
                }
                units.push(value);
            }
            0xe0..=0xef => {
                let second = *data
                    .get(pos)
                    .ok_or_else(|| invalid("truncated modified UTF-8"))?;
                let third = *data
                    .get(pos + 1)
                    .ok_or_else(|| invalid("truncated modified UTF-8"))?;
                pos += 2;
                if second & 0xc0 != 0x80 || third & 0xc0 != 0x80 {
                    return Err(invalid("invalid modified UTF-8 continuation byte"));
                }
                let value = (((first & 0x0f) as u16) << 12)
                    | (((second & 0x3f) as u16) << 6)
                    | (third & 0x3f) as u16;
                if value < 0x800 {
                    return Err(invalid("overlong modified UTF-8 sequence"));
                }
                units.push(value);
            }
            _ => return Err(invalid("invalid modified UTF-8 leading byte")),
        }
    }
    String::from_utf16(&units).map_err(|_| invalid("unpaired UTF-16 surrogate in modified UTF-8"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_named_integer_from_root_compound() {
        let input = [10, 0, 0, 3, 0, 1, b'x', 0, 0, 0, 42, 0];
        let root = parse(&input).unwrap();
        assert_eq!(root.get_int("x"), Some(42));
    }

    #[test]
    fn serializes_root_compound_and_round_trips() {
        let root = Compound(vec![("x".into(), Tag::Int(42))]);
        let encoded = write(&root).unwrap();
        assert_eq!(encoded, [10, 0, 0, 3, 0, 1, b'x', 0, 0, 0, 42, 0]);
        assert_eq!(parse(&encoded).unwrap(), root);
    }

    #[test]
    fn rejects_truncated_integer_payload() {
        let input = [10, 0, 0, 3, 0, 1, b'x', 0, 0];
        assert!(parse(&input).is_err());
    }

    #[test]
    fn rejects_trailing_bytes() {
        let input = [10, 0, 0, 0, 0];
        assert!(parse(&input).is_err());
    }

    #[test]
    fn rejects_excessively_nested_lists() {
        let mut input = vec![10, 0, 0, 9, 0, 1, b'l'];
        for _ in 0..70 {
            input.extend([9, 0, 0, 0, 1]);
        }
        input.extend([1, 0, 0, 0, 1, 42, 0, 0]);
        assert!(parse(&input).is_err());
    }

    #[test]
    fn decodes_java_modified_utf8_null() {
        let input = [10, 0, 0, 8, 0, 2, 0xc0, 0x80, 0, 0, 0];
        let root = parse(&input).unwrap();
        assert_eq!(root.0[0].0.as_bytes(), &[0]);
    }

    #[test]
    fn decodes_java_modified_utf8_surrogate_pair() {
        let input = [
            10, 0, 0, 8, 0, 6, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80, 0, 0, 0,
        ];
        let root = parse(&input).unwrap();
        assert_eq!(root.0, vec![("😀".to_string(), Tag::String(String::new()))]);
    }

    #[test]
    fn rejects_unknown_empty_list_element_type() {
        let input = [10, 0, 0, 9, 0, 1, b'l', 13, 0, 0, 0, 0, 0];
        assert!(parse(&input).is_err());
    }

    #[test]
    fn accepts_empty_list_with_end_element_type() {
        let input = [10, 0, 0, 9, 0, 1, b'l', 0, 0, 0, 0, 0, 0];
        assert!(parse(&input).is_ok());
    }

    #[test]
    fn rejects_unpaired_modified_utf8_surrogate() {
        assert!(modified_utf8(&[0xed, 0xa0, 0x80]).is_err());
    }
}
