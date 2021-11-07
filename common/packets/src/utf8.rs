use std::io::Read;

const MAX_FIELD_LEN: usize = 65535;

use crate::packet_reader::PacketError;

#[derive(Debug)]
pub struct Field {
    pub value: String,
}

impl Field {
    /// Creates a new Field struct from a string literal
    ///
    /// # Examples
    ///
    /// ```
    /// use packets::utf8::Field;
    ///
    /// let bytes: [u8; 11] = [0, 9, 116, 101, 115, 116, 32, 48, 49, 50, 51];    
    /// let msg = "test 0123";    ///
    /// let field = Field::new_from_string(msg).unwrap();
    /// assert_eq!(field.encode(), bytes);
    /// ```
    ///
    /// # Errors
    ///
    /// If value's length is greater than 65535 bytes this function returns a PacketError
    ///
    pub fn new_from_string(value: &str) -> Result<Self, PacketError> {
        if value.len() > MAX_FIELD_LEN {
            return Err(PacketError::new_msg("Largo del paquete excedido"));
        }
        Ok(Self {
            value: value.to_owned(),
        })
    }

    ///
    /// Creates a Field struct from a stream of bytes
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    ///
    /// use packets::utf8::Field;
    /// let bytes: [u8; 11] = [0, 9, 116, 101, 115, 116, 32, 48, 49, 50, 51];
    /// let msg = "test 0123";
    /// let mut cursor = Cursor::new(bytes);
    /// let field_from_stream = Field::new_from_stream(&mut cursor).unwrap();
    /// assert_eq!(field_from_stream.value, msg);
    /// ```
    ///
    pub fn new_from_stream<T: Read>(stream: &mut T) -> Option<Self> {
        let mut buf: [u8; 2] = [0; 2];
        stream.read_exact(&mut buf).ok()?;

        let size = u16::from_be_bytes(buf) as usize;
        let mut buf_string = vec![0; size];
        if stream.read_exact(&mut buf_string).is_err() {
            return None;
        }

        let value = std::str::from_utf8(&buf_string).ok()?;
        Some(Self {
            value: value.to_owned(),
        })
    }

    /// Encodes a Field struct into a UTF-8 string
    ///
    /// # Examples
    ///
    /// ```
    ///  use packets::utf8::Field;
    ///
    ///  let bytes: [u8; 11] = [0, 9, 116, 101, 115, 116, 32, 48, 49, 50, 51];
    ///  let msg = "test 0123";
    ///  let field = Field::new_from_string(msg).unwrap();
    ///  assert_eq!(field.encode(), bytes);
    /// ```
    ///
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::from(self.value.len().to_be_bytes());
        bytes.drain(0..bytes.len() - 2);

        for byte in self.value.as_bytes() {
            bytes.push(*byte);
        }
        bytes
    }

    /// Returns &str value stored in a Field struct
    pub fn decode(&self) -> &str {
        &self.value
    }

    /// Returns a Field len
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Verify if this Field is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::Field;

    #[test]
    fn test_decode() {
        let bytes: [u8; 11] = [0, 9, 116, 101, 115, 116, 32, 48, 49, 50, 51];
        let msg = "test 0123";
        let mut cursor = Cursor::new(bytes);
        let field_from_stream = Field::new_from_stream(&mut cursor).unwrap();
        assert_eq!(field_from_stream.value, msg);
    }

    #[test]
    fn test_encode() {
        let bytes: [u8; 11] = [0, 9, 116, 101, 115, 116, 32, 48, 49, 50, 51];
        let msg = "test 0123";
        let field = Field::new_from_string(msg).unwrap();
        assert_eq!(field.encode(), bytes);
    }

    #[test]
    fn test_empty_msg_returns_vec_with_two_zero_bytes_as_length() {
        let msg = "";
        let field = Field::new_from_string(msg).unwrap();
        assert_eq!(field.encode(), vec![0, 0]);
    }

    #[test]
    fn test_whitespace_character() {
        let msg = " ";
        let field = Field::new_from_string(msg).unwrap();
        assert_eq!(field.encode(), vec![0, 1, 32]);
    }
}
