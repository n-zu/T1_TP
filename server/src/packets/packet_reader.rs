use std::{
    error::Error,
    fmt,
    io::{self, Cursor, Read},
};

const CONTINUATION_SHIFT: u8 = 7;
const VALUE_MASK: u8 = 0x7f;
const CONTINUATION_MAX: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidProtocol,
    InvalidProtocolLevel,
    InvalidFlags,
    Other,
}

#[derive(Debug)]
pub struct PacketError {
    msg: String,
    kind: ErrorKind,
}

const DEFAULT_MSG: &str = "Invalid packet encoding";

impl PacketError {
    pub fn new() -> PacketError {
        PacketError {
            msg: DEFAULT_MSG.to_string(),
            kind: ErrorKind::Other,
        }
    }

    pub fn new_msg(msg: &str) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind: ErrorKind::Other,
        }
    }

    pub fn new_kind(msg: &str, kind: ErrorKind) -> PacketError {
        PacketError {
            msg: msg.to_string(),
            kind,
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
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
        PacketError::new_msg(&error.to_string())
    }
}

fn get_remaining(stream: &mut impl Read, byte: u8) -> Result<usize, PacketError> {
    let mut i: u8 = 1;
    let mut buf = [byte];
    let mut len: usize = usize::from(buf[0] & VALUE_MASK);

    while buf[0] >> CONTINUATION_SHIFT != 0 {
        if i >= CONTINUATION_MAX {
            return Err(PacketError::new_msg("Malformed Remaining Length"));
        }

        stream.read_exact(&mut buf)?;
        len += usize::from(buf[0] & VALUE_MASK) << (i * CONTINUATION_SHIFT);
        i += 1;
    }

    Ok(len)
}

pub fn read_packet_bytes(
    fixed_header: [u8; 2],
    stream: &mut impl Read,
) -> Result<Cursor<Vec<u8>>, PacketError> {
    let remaining_len = get_remaining(stream, fixed_header[1])?;

    let mut vec = vec![0u8; remaining_len];
    stream.read_exact(&mut vec)?;

    Ok(Cursor::new(vec))
}


#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::read_packet_bytes;
    use std::io::repeat;

    #[test]
    fn test_decode_short() {
        let fixed_header = [0, 10];
        let remaining : Vec<u8> = (0u8..10u8).collect();

        let mut stream = Cursor::new(remaining.clone());
        let result = read_packet_bytes(fixed_header, &mut stream);

        assert_eq!(remaining, result.unwrap().into_inner());
    }

    #[test]
    fn test_decode_2_length_bytes() {
        let fixed_header = [0, 0b10000101]; // 5 + continuation bit en primer bit de largo
        let mut vector = vec![1u8]; // 1 en segundo byte de largo
        // Total: 128 * 1 + 5 = 133
        let remaining : Vec<u8> = (0u8..133u8).collect(); // 133 bytes
        vector.append(&mut remaining.clone());

        let mut stream = Cursor::new(vector);
        let result = read_packet_bytes(fixed_header, &mut stream);

        assert_eq!(remaining, result.unwrap().into_inner());
    }

    #[test]
    fn test_decode_4_length_bytes() {
        let fixed_header = [0, 0b10000001]; // 1 + continuation bit en primer bit de largo
        let mut vector = vec![0xf3, 0xe7, 0x5c];
        /*
        Largo total: 1 + 128 * 0x73 + 128^2 * 0x67 + 128^3 * 0x5c = 0xb99f981 = 194640257 bytes
        (alrededor de 185Mb)
        */

        let mut remaining = vec![1u8; 194640257];
        vector.append(&mut remaining);

        let mut stream = Cursor::new(vector);
        let result = read_packet_bytes(fixed_header, &mut stream);

        assert_eq!(194640257, result.unwrap().into_inner().len());
    }

    #[test]
    fn test_decode_more_than_4_length_bytes_should_be_error() {
        let fixed_header = [0, 0x80];
        let mut repeat = std::io::repeat(0x80);
        // Con esto lee infinitamente 0x80, debería parar cuando llega al quinto bit de largo
        // con un error

        let result = read_packet_bytes(fixed_header, &mut repeat);

        assert!(result.is_err());
    }

}
