use crate::packet_reader;
use crate::packet_reader::PacketError;
use crate::utf8::Field;
use std::io::Read;

#[derive(Debug, PartialEq)]
pub enum QoSLevel {
    QoSLevel0,
    QoSLevel1,
}

#[derive(Debug, PartialEq)]
pub enum RetainFlag {
    RetainFlag0,
    RetainFlag1,
}

#[derive(Debug, PartialEq)]
pub enum DupFlag {
    DupFlag0,
    DupFlag1,
}

#[derive(Debug, PartialEq)]
struct Publish {
    packet_id: Option<u16>,
    topic_name: String,
    qos: QoSLevel,
    retain_flag: RetainFlag,
    dup_flag: DupFlag,
    payload: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum PublishError {
    InvalidRetainFlag,
    InvalidQoSLevel,
    InvalidDupFlag,
    InvalidControlPacketType,
    ErrorAtReadingPacket,
}

impl From<PacketError> for PublishError {
    fn from(error: PacketError) -> PublishError {
        PublishError::ErrorAtReadingPacket
    }
}

const PUBLISH_CONTROL_PACKET_TYPE: u8 = 4;

impl Publish {
    pub fn read_from(stream: &mut impl Read) -> Result<Publish, PublishError> {
        let mut first_byte_buffer = [0u8; 1];
        stream.read_exact(&mut first_byte_buffer);
        let retain_flag = Publish::verify_retain_flag(&first_byte_buffer)?;
        let qos_level = Publish::verify_qos_level_flag(&first_byte_buffer)?;
        let dup_flag = Publish::verify_dup_flag(&first_byte_buffer, &qos_level)?;
        Publish::verify_control_packet_type(&first_byte_buffer)?;
        let mut bytes = packet_reader::read_packet_bytes(stream)?;
        let topic_name = Field::new_from_stream(&mut bytes).ok_or_else(PacketError::new)?;
        let packet_id = Publish::verify_packet_id(&mut bytes, &qos_level);
        let payload = Field::new_from_stream(&mut bytes).ok_or_else(PacketError::new)?;
        Ok(Self {
            packet_id,
            topic_name: topic_name.value,
            qos: qos_level,
            retain_flag,
            dup_flag,
            payload: Option::from(payload.value),
        })
    }

    fn verify_retain_flag(first_byte_buffer: &[u8; 1]) -> Result<RetainFlag, PublishError> {
        let first_byte = first_byte_buffer[0];
        let retain_flag = first_byte & 0b1;
        match retain_flag {
            0 => Ok(RetainFlag::RetainFlag0),
            1 => Ok(RetainFlag::RetainFlag1),
            _ => Err(PublishError::InvalidRetainFlag),
        }
    }

    fn verify_qos_level_flag(first_byte_buffer: &[u8; 1]) -> Result<QoSLevel, PublishError> {
        let first_byte = first_byte_buffer[0];
        let qos_level = (first_byte & 0b110) >> 1;
        match qos_level {
            0 => Ok(QoSLevel::QoSLevel0),
            1 => Ok(QoSLevel::QoSLevel1),
            _ => Err(PublishError::InvalidQoSLevel),
        }
    }

    fn verify_dup_flag(
        first_byte_buffer: &[u8; 1],
        qos_level: &QoSLevel,
    ) -> Result<DupFlag, PublishError> {
        let first_byte = first_byte_buffer[0];
        let dup_flag = (first_byte & 0b1000) >> 3;
        if *qos_level == QoSLevel::QoSLevel0 && dup_flag == 1 {
            return Err(PublishError::InvalidDupFlag);
        }

        match dup_flag {
            0 => Ok(DupFlag::DupFlag0),
            1 => Ok(DupFlag::DupFlag1),
            _ => Err(PublishError::InvalidDupFlag),
        }
    }

    fn verify_control_packet_type(first_byte_buffer: &[u8; 1]) -> Result<(), PublishError> {
        let first_byte = first_byte_buffer[0];
        let control_packet_type = (first_byte & 0b110000) >> 4;
        if control_packet_type != PUBLISH_CONTROL_PACKET_TYPE {
            return Err(PublishError::InvalidControlPacketType);
        }
        Ok(())
    }

    fn verify_packet_id(bytes: &mut impl Read, qos_level: &QoSLevel) -> Option<u16> {
        if *qos_level != QoSLevel::QoSLevel1 {
            return None;
        }
        let mut packet_id_buffer = [0u8; 2];
        bytes.read_exact(&mut packet_id_buffer);
        let packet_id = u16::from_be_bytes(packet_id_buffer);
        Some(packet_id)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn cosas_raras() {
        let aux: u8 = 33;
        let retain_flag = aux & 0b1;
        assert_eq!(1, retain_flag);
    }

    #[test]
    fn v = []
}
