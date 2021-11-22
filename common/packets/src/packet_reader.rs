use crate::packet_error::{PacketError, PacketResult};
use std::io::{Cursor, Read};

const MAX_MULTIPLIER: usize = 128 * 128 * 128;
const MAX_VARIABLE_LENGTH: usize = 268_435_455;

/// Reads the number of bytes remaining within a stream, including data in the variable header and the payload.
pub fn read_remaining_bytes(stream: &mut impl Read) -> PacketResult<Cursor<Vec<u8>>> {
    let remaining_len = RemainingLength::from_encoded(stream)?.decode();
    let mut vec = vec![0u8; remaining_len as usize];
    stream.read_exact(&mut vec)?;
    Ok(Cursor::new(vec))
}

/// The Remaining Length is the number of bytes remaining within a stream.
///
/// The Remaining Length does not include the bytes used to encode the Remaining Length.
pub struct RemainingLength {
    length: u32,
}

impl RemainingLength {
    /// Creates a new remaining length struct from a given length
    ///
    /// # Errors
    ///
    /// This function will return a PacketError if the given length is greater than 256 MB
    pub fn from_uncoded(length: usize) -> PacketResult<Self> {
        if length > MAX_VARIABLE_LENGTH {
            return Err(PacketError::new_msg("Exceeded max variable length size"));
        }
        Ok(Self {
            length: length as u32,
        })
    }

    /// Returns the encoded remaining length from a given stream    
    pub fn from_encoded(stream: &mut impl Read) -> PacketResult<Self> {
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

    /// Encodes a length following the MQTT v3.1.1 length encoding scheme
    ///
    /// Returns a Vec<u8> which contains up to 4 bytes representing the remaining length
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

    /// Returns stored remaining length
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
