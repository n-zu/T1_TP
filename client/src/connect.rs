#![allow(dead_code)]

use packets::packet_reader::PacketError;
use packets::utf8::Field;

#[doc(hidden)]
const ENCODED_LEN_MAX_BYTES: usize = 4;
#[doc(hidden)]
const PROTOCOL_LEVEL_3_1_1: u8 = 0x04;
#[doc(hidden)]
const USER_NAME_PRESENT: u8 = 0x80;
#[doc(hidden)]
const PASSWORD_PRESENT: u8 = 0x40;
#[doc(hidden)]
const LAST_WILL_PRESENT: u8 = 0x04;
#[doc(hidden)]
const WILL_RETAIN: u8 = 0x20;
#[doc(hidden)]
const QOS_LEVEL_0: u8 = 0x00;
#[doc(hidden)]
const QOS_LEVEL_1: u8 = 0x08;
#[doc(hidden)]
const CLEAN_SESSION: u8 = 0x02;
#[doc(hidden)]
const CONNECT_PACKET_TYPE: u8 = 0x10;
#[doc(hidden)]
const CONTINUATION_SHIFT: u8 = 7;

/// Non negative integer codification algorithm for
/// variable_length, according to MQTT V3.1.1 standard
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

#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]

pub enum QoSLevel {
    QoSLevel0 = 0,
    QoSLevel1 = 1,
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
    #[doc(hidden)]
    fn protocol_name(&self) -> Vec<u8> {
        Field::new_from_string("MQTT")
            .expect("Error inesperado")
            .encode()
    }

    #[doc(hidden)]
    fn control_byte(&self) -> u8 {
        CONNECT_PACKET_TYPE
    }

    #[doc(hidden)]
    fn protocol_level(&self) -> u8 {
        PROTOCOL_LEVEL_3_1_1
    }

    #[doc(hidden)]
    fn flags(&self) -> u8 {
        let mut flags = 0;
        if self.user_name.is_some() {
            flags |= USER_NAME_PRESENT;
        }
        if self.password.is_some() {
            flags |= PASSWORD_PRESENT;
        }
        if let Some(last_will) = &self.last_will {
            if last_will.retain {
                flags |= WILL_RETAIN;
            }
            match self.last_will.as_ref().unwrap().qos {
                QoSLevel::QoSLevel0 => flags |= QOS_LEVEL_0,
                QoSLevel::QoSLevel1 => flags |= QOS_LEVEL_1,
            }
            flags |= LAST_WILL_PRESENT;
        }
        if self.clean_session {
            flags |= CLEAN_SESSION;
        }

        flags
    }

    #[doc(hidden)]
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

    #[doc(hidden)]
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header = vec![];
        variable_header.append(&mut self.protocol_name());
        variable_header.push(self.protocol_level());
        variable_header.push(self.flags());
        variable_header.push(self.keep_alive.to_be_bytes()[0]);
        variable_header.push(self.keep_alive.to_be_bytes()[1]);
        variable_header
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Vec<u8> {
        let mut fixed_header = vec![];
        let remaining_len = self.variable_header().len() + self.payload().len();
        let mut remaining_len = encode_len(remaining_len as u32);
        fixed_header.push(self.control_byte());
        fixed_header.append(&mut remaining_len);
        fixed_header
    }

    /// Encodes the content of the packet according to MQTT
    /// V3.1.1 standard
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = vec![];
        encoded.append(&mut self.fixed_header());
        encoded.append(&mut self.variable_header());
        encoded.append(&mut self.payload());
        encoded
    }
}

/// Connect packet constructor
pub struct ConnectBuilder {
    #[doc(hidden)]
    connect: Connect,
}

impl ConnectBuilder {
    /// Creates a ConnectBuilder
    ///
    /// # Errors
    ///
    /// Returns error if the length of the client_id exceedes
    /// the maximum established for utf8 fields in MQTT V3.1.1
    /// standard
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
        self.connect.password = Some(Field::new_from_string(password)?);
        Ok(self)
    }

    pub fn last_will(mut self, last_will: LastWill) -> Self {
        self.connect.last_will = Some(last_will);
        self
    }

    /// Builds the packet with the received parameters
    ///
    /// # Errors
    ///
    /// Returns error if the packet fields do not meet the
    /// requirements of the MQTT V3.1.1 standard
    pub fn build(self) -> Result<Connect, PacketError> {
        if self.connect.password.is_some() && self.connect.user_name.is_none() {
            return Err(PacketError::new_msg(
                "Se intento crear un paquete con user_name pero sin password",
            ));
        }

        Ok(self.connect)
    }
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
            [
                16, // fixed_header
                16, // remaining_length
                0, 4, 77, 81, 84, 84, // (0) (4) MQTT
                4,  // protocol_level
                2,  // flags: CLEAN_SESSION
                0, 13, // keep_alive
                0, 4, 114, 117, 115, 116 // (0) (4) rust
            ]
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
                16, // fixed_header
                26, // remaining_length
                0, 4, 77, 81, 84, 84,  // (0) (4) MQTT
                4,   // protocol_level
                194, // flags: CLEAN_SESSION | USER_NAME_PRESENT | PASSWORD_PRESENT
                0, 25, // keep_alive
                0, 4, 114, 117, 115, 116, // (0) (4) rust
                0, 2, 121, 111, // user_name: (0) (2) yo
                0, 4, 112, 97, 115, 115 // password: (0) (4) pass
            ]
        );
    }

    #[test]
    fn test_password_without_username() {
        let builder = ConnectBuilder::new("rust", 25, true)
            .unwrap()
            .password("pass")
            .unwrap();

        assert!(builder.build().is_err());
    }

    #[test]
    fn test_clean_session_false() {
        let packet = ConnectBuilder::new("rust", 25, false)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            packet.encode(),
            [
                16, // fixed_header
                16, // remaining_length
                0, 4, 77, 81, 84, 84, // (0) (4) MQTT
                4,  // protocol_level
                0,  // flags
                0, 25, // keep_alive
                0, 4, 114, 117, 115, 116 // password: (0) (4) rust
            ]
        )
    }
}
