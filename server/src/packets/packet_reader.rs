use std::{
    error::Error,
    fmt,
    io::{self, Cursor, Read},
};

const CONTINUATION_SHIFT: u8 = 7;
const VALUE_MASK: u8 = 0x3f;
const CONTINUATION_MAX: u8 = 4;

#[derive(Debug)]
pub enum ErrorKind {
    InvalidProtocolLevel,
    Other,
}

#[derive(Debug)]
pub struct PacketError {
    msg: String,
    kind: ErrorKind,
}

impl PacketError {
    pub fn new(msg: &str) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind: ErrorKind::Other,
        }
    }
    pub fn new_kind(msg: &str, kind: ErrorKind) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind: kind,
        }
    }
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for PacketError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl From<io::Error> for PacketError {
    fn from(error: io::Error) -> Self {
        PacketError::new(&error.to_string())
    }
}

fn get_remaining(stream: &mut impl Read, byte: u8) -> Result<usize, PacketError> {
    let i: u8 = 1;
    let mut buf = [byte];
    let mut len: usize = usize::from(buf[0] & VALUE_MASK);

    while buf[0] >> CONTINUATION_SHIFT != 0 {
        if i >= CONTINUATION_MAX {
            return Err(PacketError::new("Malformed Remaining Length"));
        }

        stream.read_exact(&mut buf)?;
        len += usize::from(buf[0] & VALUE_MASK) << (i * CONTINUATION_SHIFT);
    }

    return Ok(len);
}

pub fn read_packet_bytes(
    fixed_header: [u8; 2],
    stream: &mut impl Read,
) -> Result<Cursor<Vec<u8>>, PacketError> {
    let remaining_len = get_remaining(stream, fixed_header[1])?;

    let mut vec = vec![0u8; remaining_len];
    stream.read_exact(&mut vec)?;

    return Ok(Cursor::new(vec));
}
