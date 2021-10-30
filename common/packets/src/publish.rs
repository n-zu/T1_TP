#![allow(unused)]
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
pub enum PublishError {
    InvalidRetainFlag,
    InvalidQoSLevel,
    InvalidDupFlag,
    InvalidControlPacketType,
    ErrorAtReadingPacket,
    TopicNameMustBeAtLeastOneCharacterLong,
}

impl From<PacketError> for PublishError {
    fn from(error: PacketError) -> PublishError {
        PublishError::ErrorAtReadingPacket
    }
}

const PUBLISH_CONTROL_PACKET_TYPE: u8 = 3;

#[derive(Debug, PartialEq)]
pub struct Publish {
    pub packet_id: Option<u16>,
    pub topic_name: String,
    pub qos: QoSLevel,
    pub retain_flag: RetainFlag,
    pub dup_flag: DupFlag,
    pub payload: Option<String>,
}

impl Publish {
    ///
    /// Returns a Publish packet with a valid state
    ///
    /// # Arguments
    ///
    /// * `stream`: &mut impl Read
    ///
    /// returns: Result<Publish, PublishError>
    ///
    /// # Errors
    ///
    /// If the stream of bytes doesn't follow MQTT 3.1.1 protocol this function returns
    /// a PublishError corresponding to the type of the error encountered
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    /// use packets::publish::{DupFlag, QoSLevel, RetainFlag, Publish};
    /// use packets::utf8::Field;
    ///
    ///
    /// let mut first_byte: Vec<u8> = vec![0b110010]; // primer byte con los flags con QoS level 1;
    ///  let mut remaining_data: Vec<u8> = vec![];
    ///  let mut topic = Field::new_from_string("a/b").unwrap().encode();
    ///  let mut payload = Field::new_from_string("mensaje").unwrap().encode();
    ///  let mut packet_id_buf: Vec<u8> = vec![0b0, 0b1010]; // Seria 01010 = packet identifier 10;
    ///  remaining_data.append(&mut topic);
    ///  remaining_data.append(&mut packet_id_buf);
    ///  remaining_data.append(&mut payload);
    ///  let mut bytes = vec![];
    ///  bytes.append(&mut first_byte);
    ///  bytes.push(remaining_data.len() as u8);
    ///  bytes.append(&mut remaining_data);
    ///  let mut stream = Cursor::new(bytes);
    ///  let expected = Publish {
    ///             packet_id: Option::from(10 as u16),
    ///             topic_name: "a/b".to_string(),
    ///             qos: QoSLevel::QoSLevel1,
    ///             retain_flag: RetainFlag::RetainFlag0,
    ///             dup_flag: DupFlag::DupFlag0,
    ///             payload: Option::from("mensaje".to_string()),
    ///         };
    ///  let result = Publish::read_from(&mut stream).unwrap();
    ///  assert_eq!(expected, result);
    /// ```
    pub fn read_from(stream: &mut impl Read) -> Result<Publish, PublishError> {
        let mut first_byte_buffer = [0u8; 1];
        stream.read_exact(&mut first_byte_buffer);
        let retain_flag = Publish::verify_retain_flag(&first_byte_buffer)?;
        let qos_level = Publish::verify_qos_level_flag(&first_byte_buffer)?;
        let dup_flag = Publish::verify_dup_flag(&first_byte_buffer, &qos_level)?;
        Publish::verify_control_packet_type(&first_byte_buffer)?;
        let mut remaining_bytes = packet_reader::read_packet_bytes(stream)?;
        let topic_name = Publish::verify_topic_name(&mut remaining_bytes)?;
        let packet_id = Publish::verify_packet_id(&mut remaining_bytes, &qos_level);
        let payload = Field::new_from_stream(&mut remaining_bytes).ok_or_else(PacketError::new)?;
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

    fn verify_topic_name(bytes: &mut impl Read) -> Result<Field, PublishError> {
        let topic_name = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
        let topic_name_chars_count = topic_name.value.chars().count();
        if topic_name_chars_count == 0 {
            return Err(PublishError::TopicNameMustBeAtLeastOneCharacterLong);
        }
        Ok(topic_name)
    }

    pub fn packet_id(&self) -> Option<&u16> {
        self.packet_id.as_ref()
    }
    pub fn topic_name(&self) -> &str {
        &self.topic_name
    }
    pub fn qos(&self) -> &QoSLevel {
        &self.qos
    }
    pub fn retain_flag(&self) -> &RetainFlag {
        &self.retain_flag
    }
    pub fn dup_flag(&self) -> &DupFlag {
        &self.dup_flag
    }
    pub fn payload(&self) -> Option<&String> {
        self.payload.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use crate::publish::{DupFlag, Publish, PublishError, QoSLevel, RetainFlag};
    use crate::utf8::Field;
    use std::io::Cursor;

    #[test]
    fn test_dup_flag_0_with_qos_level_different_from_0_should_raise_invalid_dup_flag() {
        let first_byte: Vec<u8> = vec![0b111000];
        let mut stream = Cursor::new(first_byte);
        let expected_error = PublishError::InvalidDupFlag;
        let result = Publish::read_from(&mut stream).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_qos_level_2_should_raise_invalid_qos_level_error() {
        let first_byte: Vec<u8> = vec![0b110100];
        let mut stream = Cursor::new(first_byte);
        let expected_error = PublishError::InvalidQoSLevel;
        let result = Publish::read_from(&mut stream).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_qos_level_3_should_raise_invalid_qos_level_error() {
        let first_byte: Vec<u8> = vec![0b110110];
        let mut stream = Cursor::new(first_byte);
        let expected_error = PublishError::InvalidQoSLevel;
        let result = Publish::read_from(&mut stream).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_packet_control_type_5_should_raise_invalid_control_packet_type_error() {
        let first_byte: Vec<u8> = vec![0b100000];
        let mut stream = Cursor::new(first_byte);
        let expected_error = PublishError::InvalidControlPacketType;
        let result = Publish::read_from(&mut stream).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_publish_packet_with_qos_level_0_must_not_have_a_packet_id() {
        let mut first_byte: Vec<u8> = vec![0b110000]; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = Field::new_from_string("mensaje").unwrap().encode();
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![];
        bytes.append(&mut first_byte);
        bytes.push(remaining_data.len() as u8);
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected = Publish {
            packet_id: None,
            topic_name: "a/b".to_string(),
            qos: QoSLevel::QoSLevel0,
            retain_flag: RetainFlag::RetainFlag0,
            dup_flag: DupFlag::DupFlag0,
            payload: Option::from("mensaje".to_string()),
        };
        let result = Publish::read_from(&mut stream).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_publish_packet_with_qos_level_1_must_have_a_packet_id() {
        let mut first_byte: Vec<u8> = vec![0b110010]; // primer byte con los flags con QoS level 1;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = Field::new_from_string("mensaje").unwrap().encode();
        let mut packet_id_buf: Vec<u8> = vec![0b0, 0b1010]; // Seria 01010 = packet identifier 10;
        remaining_data.append(&mut topic);
        remaining_data.append(&mut packet_id_buf);
        remaining_data.append(&mut payload);

        let mut bytes = vec![];
        bytes.append(&mut first_byte);
        bytes.push(remaining_data.len() as u8);
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected = Publish {
            packet_id: Option::from(10 as u16),
            topic_name: "a/b".to_string(),
            qos: QoSLevel::QoSLevel1,
            retain_flag: RetainFlag::RetainFlag0,
            dup_flag: DupFlag::DupFlag0,
            payload: Option::from("mensaje".to_string()),
        };
        let result = Publish::read_from(&mut stream).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_publish_packet_might_have_zero_length_payload() {
        let mut first_byte: Vec<u8> = vec![0b110000]; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = Field::new_from_string("").unwrap().encode();
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![];
        bytes.append(&mut first_byte);
        bytes.push(remaining_data.len() as u8);
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected = Publish {
            packet_id: None,
            topic_name: "a/b".to_string(),
            qos: QoSLevel::QoSLevel0,
            retain_flag: RetainFlag::RetainFlag0,
            dup_flag: DupFlag::DupFlag0,
            payload: Option::from("".to_string()),
        };
        let result = Publish::read_from(&mut stream).unwrap();
        assert_eq!(expected, result);
        assert_eq!(expected.payload().unwrap(), "");
    }

    #[test]
    fn test_publish_packet_topics_should_be_case_sensitive() {
        let mut first_byte: Vec<u8> = vec![0b110000]; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/B").unwrap().encode();
        let mut payload = Field::new_from_string("").unwrap().encode();
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![];
        bytes.append(&mut first_byte);
        bytes.push(remaining_data.len() as u8);
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected = Publish {
            packet_id: None,
            topic_name: "a/b".to_string(),
            qos: QoSLevel::QoSLevel0,
            retain_flag: RetainFlag::RetainFlag0,
            dup_flag: DupFlag::DupFlag0,
            payload: Option::from("".to_string()),
        };
        let result = Publish::read_from(&mut stream).unwrap();
        assert_ne!(expected, result);
    }

    #[test]
    fn test_publish_packet_must_be_at_least_one_character_long() {
        let mut first_byte: Vec<u8> = vec![0b110000]; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("").unwrap().encode();
        let mut payload = Field::new_from_string("aa").unwrap().encode();
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![];
        bytes.append(&mut first_byte);
        bytes.push(remaining_data.len() as u8);
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected = PublishError::TopicNameMustBeAtLeastOneCharacterLong;
        let result = Publish::read_from(&mut stream).unwrap_err();
        assert_eq!(expected, result);
    }
}
