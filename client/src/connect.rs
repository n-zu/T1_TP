#![allow(dead_code)]

use std::io::Read;

use packets::packet_reader::PacketError;
use packets::utf8::Field;

const ENCODED_LEN_MAX_BYTES: usize = 4;

const PROTOCOL_LEVEL_3_1_1: u8 = 0x04;
const CONTINUATION_SHIFT: u8 = 7;
const VALUE_MASK: u8 = 0x3f;
const CONTINUATION_MAX: u8 = 4;

fn encode_len(len: u32) -> Vec<u8> {
    let mut len = len;
    let mut encoded_len = 0;
    while len > 0 {
        let mut encoded_byte = len % 128;
        len /= 128;
        if len > 0 {
            encoded_byte |= 128;
        }
        encoded_len <<= 8;
        encoded_len |= encoded_byte;
    }
    let mut bytes_vec = vec![];
    let bytes = encoded_len.to_be_bytes();
    let mut i = 0;
    // Salteo bytes que son 0 en caso de que la
    // codificacion no haya usado los 4 bytes
    while bytes[i] == 0 {
        i += 1;
    }
    while i < ENCODED_LEN_MAX_BYTES {
        bytes_vec.push(bytes[i]);
        i += 1
    }

    bytes_vec
}

enum QoSLevel {
    QoSLevel0,
    QoSLevel1,
}

pub struct LastWill {
    retain: bool,
    qos: QoSLevel,
    topic: Field,
    message: Field,
}

pub struct Connect {
    client_id: Field,
    clean_session: bool,
    user_name: Option<Field>,
    password: Option<Field>,
    last_will: Option<LastWill>,
    keep_alive: u16,
}

impl Connect {
    fn protocol_name(&self) -> Vec<u8> {
        Field::new_from_string("MQTT")
            .expect("Error inesperado")
            .encode()
    }

    fn control_byte(&self) -> u8 {
        0x10
    }

    fn protocol_level(&self) -> u8 {
        PROTOCOL_LEVEL_3_1_1
    }

    fn flags(&self) -> u8 {
        let mut flags = 0;
        if self.user_name.is_some() {
            flags |= 0x80;
        }
        if self.password.is_some() {
            flags |= 0x40;
        }
        if self.last_will.is_some() {
            if self.last_will.is_some() {
                flags |= 0x20;
            }
            match self.last_will.as_ref().unwrap().qos {
                QoSLevel::QoSLevel0 => flags |= 0x00,
                QoSLevel::QoSLevel1 => flags |= 0x08,
            }
            flags |= 0x04;
        }
        if self.clean_session {
            flags |= 0x02;
        }

        flags
    }

    fn payload(&self) -> Vec<u8> {
        let mut payload = vec![];
        payload.append(&mut self.client_id.encode());
        if let Some(last_will) = &self.last_will {
            payload.append(&mut last_will.topic.encode());
            payload.append(&mut last_will.message.encode());
        }
        if let Some(user_name) = &self.user_name {
            payload.append(&mut user_name.encode());
        }
        if let Some(password) = &self.password {
            payload.append(&mut password.encode());
        }
        payload
    }

    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header = vec![];
        variable_header.append(&mut self.protocol_name());
        variable_header.push(self.protocol_level());
        variable_header.push(self.flags());
        variable_header.push(self.keep_alive.to_be_bytes()[0]);
        variable_header.push(self.keep_alive.to_be_bytes()[1]);
        variable_header
    }

    fn fixed_header(&self) -> Vec<u8> {
        let mut fixed_header = vec![];
        let remaining_len = self.variable_header().len() + self.payload().len();
        let mut remaining_len = encode_len(remaining_len as u32);
        fixed_header.push(self.control_byte());
        fixed_header.append(&mut remaining_len);
        fixed_header
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = vec![];
        encoded.append(&mut self.fixed_header());
        encoded.append(&mut self.variable_header());
        encoded.append(&mut self.payload());
        encoded
    }
}

impl LastWill {
    fn new(
        retain: bool,
        qos: QoSLevel,
        topic: &str,
        message: &str,
    ) -> Result<LastWill, PacketError> {
        Ok(Self {
            retain,
            qos,
            topic: Field::new_from_string(topic)?,
            message: Field::new_from_string(message)?,
        })
    }
}

pub struct ConnectBuilder {
    connect: Connect,
}

impl ConnectBuilder {
    pub fn new(client_id: &str, keep_alive: u16, clean_session: bool) -> Result<Self, PacketError> {
        Ok(ConnectBuilder {
            connect: Connect {
                client_id: Field::new_from_string(client_id)?,
                clean_session,
                user_name: None,
                password: None,
                last_will: None,
                keep_alive,
            },
        })
    }

    pub fn user_name(mut self, user_name: &str) -> Result<Self, PacketError> {
        self.connect.user_name = Some(Field::new_from_string(user_name)?);
        Ok(self)
    }

    pub fn password(mut self, password: &str) -> Result<Self, PacketError> {
        //check_payload_field_length(&password)?;
        self.connect.password = Some(Field::new_from_string(password)?);
        Ok(self)
    }

    pub fn last_will(mut self, last_will: LastWill) -> Self {
        self.connect.last_will = Some(last_will);
        self
    }

    pub fn build(self) -> Result<Connect, PacketError> {
        if self.connect.password.is_some() && self.connect.user_name.is_none() {
            return Err(PacketError::new_msg(
                "Se intento crear un paquete con user_name pero sin password",
            ));
        }

        Ok(self.connect)
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    InvalidProtocolLevel,
    Other,
}

fn get_remaining(stream: &mut impl Read, byte: u8) -> Result<usize, PacketError> {
    let i: u8 = 1;
    let mut buf = [byte];
    let mut len: usize = usize::from(buf[0] & VALUE_MASK);

    while buf[0] >> CONTINUATION_SHIFT != 0 {
        if i >= CONTINUATION_MAX {
            return Err(PacketError::new_msg("Malformed Remaining Length"));
        }

        stream.read_exact(&mut buf)?;
        len += usize::from(buf[0] & VALUE_MASK) << (i * CONTINUATION_SHIFT);
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::ConnectBuilder;

    #[test]
    fn test_basics() {
        let packet = ConnectBuilder::new("rust", 13, true)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            packet.encode(),
            [16, 16, 0, 4, 77, 81, 84, 84, 4, 2, 0, 13, 0, 4, 114, 117, 115, 116]
        );
    }

    #[test]
    fn test_username_password() {
        let packet = ConnectBuilder::new("rust", 25, true)
            .unwrap()
            .user_name("yo")
            .unwrap()
            .password("pass")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            packet.encode(),
            [
                16, 26, 0, 4, 77, 81, 84, 84, 4, 194, 0, 25, 0, 4, 114, 117, 115, 116, 0, 2, 121,
                111, 0, 4, 112, 97, 115, 115
            ]
        );
    }

    #[test]
    #[should_panic]
    fn test_password_without_username() {
        let _ = ConnectBuilder::new("rust", 25, true)
            .unwrap()
            .password("pass")
            .unwrap()
            .build()
            .unwrap();
    }

    #[test]
    fn test_clean_session_false() {
        let packet = ConnectBuilder::new("rust", 25, false)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            packet.encode(),
            [16, 16, 0, 4, 77, 81, 84, 84, 4, 0, 0, 25, 0, 4, 114, 117, 115, 116]
        )
    }
}
