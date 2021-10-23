use std::io::Read;

pub struct Connack {}

impl Connack {
    pub fn new(_fixed_header: [u8; 2], _stream: impl Read) -> Result<Connack, String> {
        Ok(Connack {})
    }
}
