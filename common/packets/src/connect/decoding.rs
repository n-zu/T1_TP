use std::{convert::TryFrom, io::Read};

use super::*;
use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    packet_reader,
    qos::QoSLevel,
    traits::MQTTDecoding,
    utf8::Field,
};

#[doc(hidden)]
const RESERVED_BITS_FLAG_MASK: u8 = 0b00000001;
#[doc(hidden)]
const RESERVED_BITS_FLAG: u8 = 0b00000000;
#[doc(hidden)]
const CLEAN_SESSION: u8 = 0x2;
#[doc(hidden)]
const WILL_QOS: u8 = 0x18;

impl MQTTDecoding for Connect {
    fn read_from(stream: &mut impl Read, control_byte: u8) -> PacketResult<Connect> {
        let mut bytes = packet_reader::read_remaining_bytes(stream)?;
        check_packet_type(control_byte, PacketType::Connect)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        Connect::verify_protocol(&mut bytes)?;
        Connect::verify_protocol_level(&mut bytes)?;
        let mut ret = Connect::get_flags(&mut bytes)?;
        ret.get_keepalive(&mut bytes)?;
        ret.get_clientid(&mut bytes)?;
        ret.get_will_data(&mut bytes)?;
        ret.get_auth(&mut bytes)?;

        let mut buf = [0u8; 1];
        match bytes.read(&mut buf) {
            Ok(1) | Err(_) => {
                // Sobraron bytes, no debería
                return Err(PacketError::new());
            }
            Ok(_) => (), // No sobro, perfecto
        }

        Ok(ret)
    }
}

impl Connect {
    fn verify_protocol(bytes: &mut impl Read) -> PacketResult<()> {
        match Field::new_from_stream(bytes) {
            Some(mensaje) if mensaje.value != "MQTT" => Err(PacketError::new_kind(
                "Invalid protocol",
                ErrorKind::InvalidProtocol,
            )),
            None => Err(PacketError::new()),
            Some(_mensaje) => Ok(()),
        }
    }

    fn verify_protocol_level(bytes: &mut impl Read) -> PacketResult<()> {
        let mut buf = [0; 1];
        bytes.read_exact(&mut buf)?;
        if buf[0] != PROTOCOL_LEVEL_3_1_1 {
            return Err(PacketError::new_kind(
                "Invalid protocol level",
                ErrorKind::InvalidProtocolLevel,
            ));
        }
        Ok(())
    }

    fn get_will(buf: [u8; 1]) -> PacketResult<Option<LastWill>> {
        let qos = QoSLevel::try_from((buf[0] & WILL_QOS) >> WILL_QOS_SHIFT)?;
        if buf[0] & LAST_WILL_PRESENT != 0 {
            return Ok(Some(LastWill {
                retain: buf[0] & WILL_RETAIN != 0,
                qos,
                topic: String::new(),
                message: String::new(),
            }));
        }

        if buf[0] & WILL_RETAIN != 0 || qos != QoSLevel::QoSLevel0 {
            return Err(PacketError::new_kind(
                "Invalid flags",
                ErrorKind::InvalidFlags,
            ));
        }
        Ok(None)
    }

    fn get_flags(bytes: &mut impl Read) -> PacketResult<Connect> {
        let mut buf = [0; 1];
        bytes.read_exact(&mut buf)?;

        if (buf[0] & RESERVED_BITS_FLAG_MASK) != RESERVED_BITS_FLAG {
            return Err(PacketError::new_kind(
                "Reserved bits are not zero",
                ErrorKind::InvalidFlags,
            ));
        }

        Ok(Connect {
            client_id: String::new(),
            clean_session: buf[0] & CLEAN_SESSION != 0,
            user_name: if buf[0] & USER_NAME_PRESENT != 0 {
                Some(String::new())
            } else {
                None
            },
            password: if buf[0] & PASSWORD_PRESENT != 0 {
                Some(String::new())
            } else {
                None
            },
            last_will: Connect::get_will(buf)?,
            keep_alive: 0,
        })
    }

    fn get_keepalive(&mut self, bytes: &mut impl Read) -> PacketResult<()> {
        let mut buf = [0; 2];
        bytes.read_exact(&mut buf)?;
        self.keep_alive = u16::from_be_bytes(buf);
        Ok(())
    }

    fn get_clientid(&mut self, bytes: &mut impl Read) -> PacketResult<()> {
        let string = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
        self.client_id = string.value;
        Ok(())
    }

    fn get_will_data(&mut self, bytes: &mut impl Read) -> PacketResult<()> {
        if let Some(lw) = &mut self.last_will {
            let topic = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
            let message = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
            lw.topic = topic.value;
            lw.message = message.value;
        }
        Ok(())
    }

    fn get_auth(&mut self, bytes: &mut impl Read) -> PacketResult<()> {
        if let Some(user_name) = &mut self.user_name {
            let user = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
            *user_name = user.value;
        }
        if let Some(pw) = &mut self.password {
            let password = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
            *pw = password.value;
        }
        Ok(())
    }

    /***************
    WIP
    ****************/
    pub fn new_from_zero(stream: &mut impl Read) -> PacketResult<Connect> {
        let mut control_byte_buff: [u8; 1] = [0];
        stream.read_exact(&mut control_byte_buff)?;
        Connect::read_from(stream, control_byte_buff[0])
    }
}
