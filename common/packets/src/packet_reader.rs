use std::{
    convert::TryFrom,
    error::Error,
    fmt,
    io::{self, Cursor, Read},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum QoSLevel {
    QoSLevel0,
    QoSLevel1,
    QoSLevel2,
}

impl From<QoSLevel> for u8 {
    fn from(qos: QoSLevel) -> u8 {
        match qos {
            QoSLevel::QoSLevel0 => 0,
            QoSLevel::QoSLevel1 => 1,
            QoSLevel::QoSLevel2 => 2,
        }
    }
}

impl TryFrom<u8> for QoSLevel {
    type Error = PacketError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(QoSLevel::QoSLevel0),
            1 => Ok(QoSLevel::QoSLevel1),
            2 => Ok(QoSLevel::QoSLevel2),
            _ => Err(PacketError::new_kind(
                "Invalid QoS level",
                ErrorKind::InvalidQoSLevel,
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidProtocol,
    InvalidProtocolLevel,
    InvalidFlags,
    InvalidReservedBytes,
    ClientDisconnected,
    InvalidQoSLevel,
    InvalidDupFlag,
    InvalidControlPacketType,
    ErrorAtReadingPacket,
    TopicNameMustBeAtLeastOneCharacterLong,
    TopicNameMustNotHaveWildcards,
    InvalidReturnCode,
    WouldBlock,
    UnexpectedEof,
    Other,
}

#[derive(Debug, PartialEq)]
pub struct PacketError {
    msg: String,
    kind: ErrorKind,
}

impl Default for PacketError {
    fn default() -> Self {
        Self::new()
    }
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
        match error.kind() {
            io::ErrorKind::UnexpectedEof => {
                PacketError::new_kind(&error.to_string(), ErrorKind::UnexpectedEof)
            }
            io::ErrorKind::WouldBlock => {
                PacketError::new_kind(&error.to_string(), ErrorKind::WouldBlock)
            }
            _ => PacketError::new_msg(&error.to_string()),
        }
    }
}

impl From<PacketError> for String {
    fn from(error: PacketError) -> Self {
        error.to_string()
    }
}

pub fn read_packet_bytes(stream: &mut impl Read) -> Result<Cursor<Vec<u8>>, PacketError> {
    let remaining_len = RemainingLength::from_encoded(stream)?.decode();
    let mut vec = vec![0u8; remaining_len as usize];
    stream.read_exact(&mut vec)?;
    Ok(Cursor::new(vec))
}

const MAX_MULTIPLIER: usize = 128 * 128 * 128;
const MAX_VARIABLE_LENGTH: usize = 268_435_455;

pub struct RemainingLength {
    length: u32,
}

impl RemainingLength {
    pub fn from_uncoded(length: usize) -> Result<Self, PacketError> {
        if length > MAX_VARIABLE_LENGTH {
            return Err(PacketError::new_msg("Exceeded max variable length size"));
        }
        Ok(Self {
            length: length as u32,
        })
    }

    pub fn from_encoded(stream: &mut impl Read) -> Result<Self, PacketError> {
        let mut multiplier: u32 = 1;
        let mut length: u32 = 0;
        loop {
            let mut buff: [u8; 1] = [0; 1];
            stream.read_exact(&mut buff)?;
            let encoded_byte = buff[0];
            length += (encoded_byte & 127) as u32 * multiplier;
            multiplier *= 128;
            if encoded_byte & 128 == 0 {
                break;
            }
            if multiplier as usize > MAX_MULTIPLIER {
                return Err(PacketError::new_msg("Malformed Remaining Length"));
            }
        }
        Ok(Self { length })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut length = self.length;
        let mut encoded_len = vec![];
        loop {
            let mut encoded_byte = length % 128;
            length /= 128;
            if length > 0 {
                encoded_byte |= 128;
            }
            encoded_len.push(encoded_byte as u8);
            if length == 0 {
                break;
            }
        }
        encoded_len
    }

    pub fn decode(&self) -> u32 {
        self.length
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::RemainingLength;

    #[test]
    fn test_encode() {
        let remaining_length = RemainingLength::from_uncoded(64).unwrap();
        assert_eq!(remaining_length.encode(), vec![0x40]);
    }

    #[test]
    fn test_encode_2_length_bytes() {
        let remaining_length = RemainingLength::from_uncoded(321).unwrap();
        assert_eq!(remaining_length.encode(), vec![0xC1, 0x02]);
    }

    #[test]
    fn test_encode_4_length_bytes() {
        let remaining_length = RemainingLength::from_uncoded(268_435_455).unwrap();
        assert_eq!(remaining_length.encode(), vec![0xFF, 0xFF, 0xFF, 0x7F]);
    }

    #[test]
    fn test_decode_short() {
        let mut bytes = vec![10, 10];
        bytes.append(&mut (0u8..10u8).collect());

        let mut stream = Cursor::new(bytes);
        let remaining = RemainingLength::from_encoded(&mut stream).unwrap();

        assert_eq!(10, remaining.decode());
    }

    #[test]
    fn test_decode_2_length_bytes() {
        let mut bytes = vec![0b10000101]; // 5 + continuation bit en primer bit de largo
        bytes.push(1u8); // 1 en segundo byte de largo
                         // Total: 128 * 1 + 5 = 133
        bytes.append(&mut (0u8..133u8).collect()); // 133 bytes

        let mut stream = Cursor::new(bytes);
        let remaining = RemainingLength::from_encoded(&mut stream).unwrap();

        assert_eq!(133, remaining.decode());
    }

    #[test]
    fn test_decode_4_length_bytes() {
        let mut bytes = vec![0b10000001]; // 1 + continuation bit en primer bit de largo
        bytes.append(&mut vec![0xf3, 0xe7, 0x5c]);
        /*
        Largo total: 1 + 128 * 0x73 + 128^2 * 0x67 + 128^3 * 0x5c = 0xb99f981 = 194640257 bytes
        (alrededor de 185Mb)
        */
        bytes.append(&mut vec![1u8; 194640257]);

        let mut stream = Cursor::new(bytes);
        let remaining = RemainingLength::from_encoded(&mut stream).unwrap();

        assert_eq!(194640257, remaining.decode());
    }

    #[test]
    fn test_decode_more_than_4_length_bytes_should_be_error() {
        let mut bytes = vec![0x80];
        bytes.append(&mut [0x80].repeat(1000));
        // Debería parar cuando llega al quinto bit de largo
        // con un error

        let mut stream = Cursor::new(bytes);
        let remaining = RemainingLength::from_encoded(&mut stream);

        assert!(remaining.is_err());
    }
}
