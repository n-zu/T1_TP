use std::io::{Read};

use self::packet_reader::PacketError;
mod packet_reader;
#[path = "../utf8.rs"]
mod utf8;
use crate::connect::utf8::Field;

/* 
const MAX_PAYLOAD_FIELD_LEN: usize = 65535;
const CONNECT_FIXED_HEADER_TYPE: u8 = 0x01;
const CONNECT_FIXED_HEADER_FLAGS: u8 = 0x00;
const SHIFT: u8 = 4;
const PROTOCOL_LEVEL_3_1_1: u8 = 0x04;
*/

enum QoSLevel {
    QoSLevel0,
    QoSLevel1,
}

struct LastWill {
    retain: bool,
    qos: QoSLevel,
    message: String,
}

pub struct Connect {
    client_id: String,
    clean_session: bool,
    user_name: Option<String>,
    password: Option<String>,
    last_will: Option<LastWill>,
    keep_alive: u16,
}

const PROTOCOL_LEVEL: u8 = 4;

impl Connect {
    fn verify_protocol(bytes: &mut impl Read) -> Result<(), PacketError> {
        match Field::new_from_stream(bytes) {
            Some(mensaje) if mensaje.value != "MQTT" => {
                return Err(PacketError::new("Invalid protocol"));
            }
            None => {
                return Err(PacketError::new("Invalid packet encoding"));
            }
            Some(_mensaje) => Ok(()),
        }
    }

    fn verify_protocol_level(bytes: &mut impl Read) -> Result<(), PacketError> {
        let mut buf = [0; 1];
        match bytes.read_exact(&mut buf) {
            Ok(()) if buf[0] != PROTOCOL_LEVEL => {
                return Err(PacketError::new_kind(
                    "Invalid protocol level",
                    packet_reader::ErrorKind::InvalidProtocolLevel,
                ));
            }
            Err(err) => {
                return Err(PacketError::new(&err.to_string()));
            }
            Ok(()) => Ok(()),
        }
    }

    pub fn new(fixed_header: [u8; 2], stream: &mut impl Read) -> Result<Connect, PacketError> {
        let mut bytes = packet_reader::read_packet_bytes(fixed_header, stream)?;

        Connect::verify_protocol(&mut bytes)?;
        Connect::verify_protocol_level(&mut bytes)?;


        Err(PacketError::new("error temporal"))
    }

    pub fn response(&self) {
        //TODO
    }

}

