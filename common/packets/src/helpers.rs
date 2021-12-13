use std::{convert::TryFrom, fmt};

use crate::packet_error::{ErrorKind, PacketError, PacketResult};

const PACKET_TYPE_MASK: u8 = 0b11110000;
const PACKET_TYPE_SHIFT: u8 = 4;

const CONNECT_PACKET_TYPE_BITS: u8 = 1;
const CONNACK_PACKET_TYPE_BITS: u8 = 2;
const PUBLISH_PACKET_TYPE_BITS: u8 = 3;
const PUBACK_PACKET_TYPE_BITS: u8 = 4;
const PUBREC_PACKET_TYPE_BITS: u8 = 5;
const PUBREL_PACKET_TYPE_BITS: u8 = 6;
const PUBCOMP_PACKET_TYPE_BITS: u8 = 7;
const SUBSCRIBE_PACKET_TYPE_BITS: u8 = 8;
const SUBACK_PACKET_TYPE_BITS: u8 = 9;
const UNSUBSCRIBE_PACKET_TYPE_BITS: u8 = 10;
const UNSUBACK_PACKET_TYPE_BITS: u8 = 11;
const PINGREQ_PACKET_TYPE_BITS: u8 = 12;
const PINGRESP_PACKET_TYPE_BITS: u8 = 13;
const DISCONNECT_PACKET_TYPE_BITS: u8 = 14;

const RESERVED_BITS_MASK: u8 = 0b00001111;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PacketType {
    Connect,
    Connack,
    Publish,
    Puback,
    PubRec,
    PubRel,
    PubComp,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    PingReq,
    PingResp,
    Disconnect,
}

impl TryFrom<u8> for PacketType {
    type Error = PacketError;

    fn try_from(control_byte: u8) -> Result<Self, Self::Error> {
        let packet_type_bytes = (control_byte & PACKET_TYPE_MASK) >> PACKET_TYPE_SHIFT;
        match packet_type_bytes {
            CONNECT_PACKET_TYPE_BITS => Ok(PacketType::Connect),
            CONNACK_PACKET_TYPE_BITS => Ok(PacketType::Connack),
            PUBLISH_PACKET_TYPE_BITS => Ok(PacketType::Publish),
            PUBACK_PACKET_TYPE_BITS => Ok(PacketType::Puback),
            PUBREC_PACKET_TYPE_BITS => Ok(PacketType::PubRec),
            PUBREL_PACKET_TYPE_BITS => Ok(PacketType::PubRel),
            PUBCOMP_PACKET_TYPE_BITS => Ok(PacketType::PubComp),
            SUBSCRIBE_PACKET_TYPE_BITS => Ok(PacketType::Subscribe),
            SUBACK_PACKET_TYPE_BITS => Ok(PacketType::Suback),
            UNSUBSCRIBE_PACKET_TYPE_BITS => Ok(PacketType::Unsubscribe),
            UNSUBACK_PACKET_TYPE_BITS => Ok(PacketType::Unsuback),
            PINGREQ_PACKET_TYPE_BITS => Ok(PacketType::PingReq),
            PINGRESP_PACKET_TYPE_BITS => Ok(PacketType::PingResp),
            DISCONNECT_PACKET_TYPE_BITS => Ok(PacketType::Disconnect),
            forbidden => Err(PacketError::new_kind(
                &format!(
                    "Tipo de paquete invalido encontrado en el control_byte ({})",
                    forbidden
                ),
                ErrorKind::InvalidControlPacketType,
            )),
        }
    }
}

impl From<PacketType> for u8 {
    fn from(packet_type: PacketType) -> Self {
        match packet_type {
            PacketType::Connect => CONNECT_PACKET_TYPE_BITS,
            PacketType::Connack => CONNACK_PACKET_TYPE_BITS,
            PacketType::Publish => PUBLISH_PACKET_TYPE_BITS,
            PacketType::Puback => PUBACK_PACKET_TYPE_BITS,
            PacketType::PubRec => PUBREC_PACKET_TYPE_BITS,
            PacketType::PubRel => PUBREL_PACKET_TYPE_BITS,
            PacketType::PubComp => PUBCOMP_PACKET_TYPE_BITS,
            PacketType::Subscribe => SUBSCRIBE_PACKET_TYPE_BITS,
            PacketType::Suback => SUBACK_PACKET_TYPE_BITS,
            PacketType::Unsubscribe => UNSUBSCRIBE_PACKET_TYPE_BITS,
            PacketType::Unsuback => UNSUBACK_PACKET_TYPE_BITS,
            PacketType::PingReq => PINGREQ_PACKET_TYPE_BITS,
            PacketType::PingResp => PINGRESP_PACKET_TYPE_BITS,
            PacketType::Disconnect => DISCONNECT_PACKET_TYPE_BITS,
        }
    }
}

#[inline(always)]
pub fn compare_reserved_bytes(control_byte: u8, expected_reserved_bits: u8) -> bool {
    (control_byte & RESERVED_BITS_MASK) == expected_reserved_bits
}

pub fn check_packet_type(control_byte: u8, expected_packet_type: PacketType) -> PacketResult<()> {
    let packet_type = PacketType::try_from(control_byte)?;
    if packet_type != expected_packet_type {
        Err(PacketError::new_kind(
            "Tipo de paquete invalido al crear paquete",
            ErrorKind::InvalidControlPacketType,
        ))
    } else {
        Ok(())
    }
}

pub fn check_reserved_bits(control_byte: u8, expected_reserved_bits: u8) -> PacketResult<()> {
    if compare_reserved_bytes(control_byte, expected_reserved_bits) {
        Ok(())
    } else {
        Err(PacketError::new_kind(
            "Bits reservados invalidos",
            ErrorKind::InvalidReservedBits,
        ))
    }
}

#[inline(always)]
pub fn build_control_byte(packet_type: PacketType, reserved_bits: u8) -> u8 {
    ((u8::from(packet_type)) << PACKET_TYPE_SHIFT) | reserved_bits
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let packet_str = match self {
            PacketType::Connect => "CONNECT",
            PacketType::Connack => "CONNACK",
            PacketType::Publish => "PUBLISH",
            PacketType::Puback => "SUBSCRIBE",
            PacketType::PubRec => "PUBREC",
            PacketType::PubRel => "PUBREL",
            PacketType::PubComp => "PUBCOMP",
            PacketType::Subscribe => "SUBSCRIBE",
            PacketType::Suback => "SUBACK",
            PacketType::Unsubscribe => "UNSUBSCRIBE",
            PacketType::Unsuback => "UNSUBACK",
            PacketType::PingReq => "PINGREQ",
            PacketType::PingResp => "PINGRESP",
            PacketType::Disconnect => "DISCONNECT",
        };
        write!(f, "{}", &packet_str)
    }
}

#[cfg(test)]
mod tests {
    use crate::helpers::PacketType;

    use super::build_control_byte;

    #[test]
    fn test_build_connect_control_byte() {
        let control_byte = build_control_byte(PacketType::Connect, 0);
        assert_eq!(control_byte, 0b00010000);
    }
}
