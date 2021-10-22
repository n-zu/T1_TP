use std::io::Read;

pub(crate) struct Field {
    pub value: String,
}

impl Field {
    pub fn new_from_string(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }

    pub fn new_from_stream<T: Read>(stream: &mut T) -> Option<Self> {
        let mut buf: [u8; 2] = [0; 2];
        stream.read_exact(&mut buf).unwrap();
        let mut buf_u32: [u8; 8] = [0; 8];
        buf_u32[6] = buf[0];
        buf_u32[7] = buf[1];

        let size = usize::from_be_bytes(buf_u32);
        let mut buf_string = vec![0; size];
        if stream.read_exact(&mut buf_string).is_err() {
            return None;
        }
        let value = std::str::from_utf8(&buf_string).unwrap();
        Some(Self {
            value: value.to_owned(),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let len_bytes = self.value.len().to_be_bytes();
        for i in len_bytes.len() - 2..len_bytes.len() {
            bytes.push(len_bytes[i]);
        }

        for byte in self.value.as_bytes() {
            println!("{}", byte);
            bytes.push(*byte);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::connect::utf8::Field;

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
        let field = Field::new_from_string(msg);
        assert_eq!(field.encode(), bytes);
    }
}
