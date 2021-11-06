use crate::packet_reader::{self, QoSLevel, RemainingLength};
use crate::packet_reader::{ErrorKind, PacketError};
use crate::utf8::Field;
use std::convert::TryInto;
use std::io::Read;

#[doc(hidden)]
const RETAIN_FLAG: u8 = 0b00000001;
#[doc(hidden)]
const QOS_SHIFT: u8 = 1;
#[doc(hidden)]
const DUP_FLAG: u8 = 0b00001000;
#[doc(hidden)]
const PUBLISH_PACKET_TYPE: u8 = 0b00110000;
#[doc(hidden)]
const SINGLE_LEVEL_WILDCARD: char = '+';
#[doc(hidden)]
const MULTI_LEVEL_WILDCARD: char = '#';
#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";

#[derive(Debug, PartialEq, Clone)]
pub enum RetainFlag {
    RetainFlag0,
    RetainFlag1,
}

#[derive(Debug, PartialEq, Clone)]
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

#[doc(hidden)]
const PUBLISH_CONTROL_PACKET_TYPE: u8 = 3;

#[derive(Debug, PartialEq, Clone)]
/// Publish packet structure for server/client side
pub struct Publish {
    packet_id: Option<u16>,
    topic_name: String,
    qos: QoSLevel,
    retain_flag: RetainFlag,
    dup_flag: DupFlag,
    payload: Option<String>,
}

impl Clone for Publish {
    fn clone(&self) -> Self {
        Self {
            packet_id: self.packet_id.clone(),
            topic_name: self.topic_name.clone(),
            qos: self.qos.clone(),
            retain_flag: self.retain_flag.clone(),
            dup_flag: self.dup_flag.clone(),
            payload: self.payload.clone(),
        }
    }
}

impl Publish {
    #![allow(dead_code)]
    ///
    /// Returns a Publish packet with a valid state
    ///
    /// It assumes the first byte is read by the server/client
    ///
    ///
    /// # Errors
    ///
    /// If the stream of bytes doesn't follow MQTT 3.1.1 protocol this function returns
    /// a PacketError corresponding to the type of the error encountered
    ///
<<<<<<< HEAD
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    /// use packets::packet_reader::QoSLevel;
    /// use packets::publish::{DupFlag, RetainFlag, Publish};
    /// use packets::utf8::Field;
    ///
    ///  let control_byte = 0b110010; // primer byte con los flags con QoS level 1;
    ///  let mut remaining_data: Vec<u8> = vec![];
    ///  let mut topic = Field::new_from_string("a/b").unwrap().encode();
    ///  let mut payload = Vec::from("mensaje".as_bytes());
    ///  let mut packet_id_buf: Vec<u8> = vec![0b0, 0b1010]; // Seria 01010 = packet identifier 10;
    ///  remaining_data.append(&mut topic);
    ///  remaining_data.append(&mut packet_id_buf);
    ///  remaining_data.append(&mut payload);
    ///  let mut bytes = vec![];
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
    ///  let result = Publish::read_from(&mut stream, control_byte).unwrap();
    ///  assert_eq!(expected, result);
    /// ```
=======
>>>>>>> 5cf7757ddac8d0e08aea518ba4f776eabe4c74c8
    pub fn read_from(stream: &mut impl Read, control_byte: u8) -> Result<Publish, PacketError> {
        let retain_flag = Self::verify_retain_flag(&control_byte);
        let qos_level = Self::verify_qos_level_flag(&control_byte)?;
        let dup_flag = Self::verify_dup_flag(&control_byte, &qos_level)?;
        Self::verify_control_packet_type(&control_byte)?;
        let mut remaining_bytes = packet_reader::read_packet_bytes(stream)?;
        let topic_name = Self::verify_topic_name(&mut remaining_bytes)?;
        let packet_id = Self::verify_packet_id(&mut remaining_bytes, &qos_level)?;
        let payload = Self::read_payload(&mut remaining_bytes);

        Ok(Self {
            packet_id,
            topic_name: topic_name.value,
            qos: qos_level,
            retain_flag,
            dup_flag,
            payload: Option::from(payload),
        })
    }

    #[doc(hidden)]
    fn verify_retain_flag(control_byte: &u8) -> RetainFlag {
        let retain_flag = control_byte & 0b1;
        if retain_flag == 0 {
            RetainFlag::RetainFlag0
        } else {
            RetainFlag::RetainFlag1
        }
    }

    #[doc(hidden)]
    fn verify_qos_level_flag(control_byte: &u8) -> Result<QoSLevel, PacketError> {
        let qos_level = (control_byte & 0b110) >> 1;
        qos_level.try_into()
    }

    #[doc(hidden)]
    fn verify_dup_flag(control_byte: &u8, qos_level: &QoSLevel) -> Result<DupFlag, PacketError> {
        let dup_flag = (control_byte & 0b1000) >> 3;
        if *qos_level == QoSLevel::QoSLevel0 && dup_flag == 1 {
            return Err(PacketError::new_kind(
                "It can not be dup flag 1 with QoS level 0",
                ErrorKind::InvalidDupFlag,
            ));
        }
        if dup_flag == 0 {
            Ok(DupFlag::DupFlag0)
        } else {
            Ok(DupFlag::DupFlag1)
        }
    }

    #[doc(hidden)]
    fn verify_control_packet_type(control_byte: &u8) -> Result<(), PacketError> {
        let control_packet_type = (control_byte & 0b11110000) >> 4;
        if control_packet_type != PUBLISH_CONTROL_PACKET_TYPE {
            return Err(PacketError::new_kind(
                "Packet type must be 3 for a Publish packet",
                ErrorKind::InvalidControlPacketType,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_packet_id(
        bytes: &mut impl Read,
        qos_level: &QoSLevel,
    ) -> Result<Option<u16>, PacketError> {
        if *qos_level != QoSLevel::QoSLevel1 {
            return Ok(None);
        }
        let mut packet_id_buffer = [0u8; 2];
        bytes.read_exact(&mut packet_id_buffer);
        let packet_id = u16::from_be_bytes(packet_id_buffer);
        if packet_id == 0 {
            return Err(PacketError::new_msg(MSG_INVALID_PACKET_ID));
        }
        Ok(Some(packet_id))
    }

    #[doc(hidden)]
    fn verify_topic_name(bytes: &mut impl Read) -> Result<Field, PacketError> {
        let topic_name = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
        let topic_name_chars_count = topic_name.value.chars().count();
        if topic_name_chars_count == 0 {
            return Err(PacketError::new_kind(
                "Topic name must be at least one character long for a Publish packet",
                ErrorKind::TopicNameMustBeAtLeastOneCharacterLong,
            ));
        }
        Ok(topic_name)
    }

    /// Gets packet_id from a Publish packet
    pub fn packet_id(&self) -> Option<&u16> {
        self.packet_id.as_ref()
    }
    /// Gets topic_name from a Publish packet
    pub fn topic_name(&self) -> &str {
        &self.topic_name
    }
    /// Gets QoS from a Publish packet
    pub fn qos(&self) -> &QoSLevel {
        &self.qos
    }
    /// Gets retain_flag from a Publish packet
    pub fn retain_flag(&self) -> &RetainFlag {
        &self.retain_flag
    }
    /// Gets dup_flag from a Publish packet
    pub fn dup_flag(&self) -> &DupFlag {
        &self.dup_flag
    }
    /// Gets payload from a Publish packet
    pub fn payload(&self) -> Option<&String> {
        self.payload.as_ref()
    }

    #[doc(hidden)]
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header = vec![];
        variable_header.append(&mut Field::new_from_string(&self.topic_name).unwrap().encode());
        if let Some(packet_identifier) = self.packet_id {
            variable_header.push(packet_identifier.to_be_bytes()[0]);
            variable_header.push(packet_identifier.to_be_bytes()[1]);
        }

        variable_header
    }

    #[doc(hidden)]
    fn control_byte(&self) -> u8 {
        let mut control_byte = PUBLISH_PACKET_TYPE;
        if self.dup_flag == DupFlag::DupFlag1 {
            control_byte |= DUP_FLAG;
        }

        control_byte |= (self.qos as u8) << QOS_SHIFT;

        if self.retain_flag == RetainFlag::RetainFlag1 {
            control_byte |= RETAIN_FLAG;
        }
        control_byte
    }

    #[doc(hidden)]
    fn encoded_payload(&self) -> Vec<u8> {
        match self.payload.as_ref() {
            Some(payload) => Vec::from(payload.as_bytes()),
            None => vec![],
        }
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Result<Vec<u8>, PacketError> {
        let mut fixed_header = vec![];
        let variable_header_len = self.variable_header().len();
        let message_len = self.encoded_payload().len();
        let remaining_length = RemainingLength::from_uncoded(variable_header_len + message_len)?;
        let control_byte = self.control_byte();
        fixed_header.push(control_byte);
        fixed_header.append(&mut remaining_length.encode());
        Ok(fixed_header)
    }

    /// Sets the QoS of this packet to max_qos if the current level is greater than it
    pub fn set_max_qos(&mut self, max_qos: QoSLevel) {
        if (max_qos as u8) < (self.qos as u8) {
            self.qos = max_qos;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::packet_reader::{ErrorKind, PacketError, QoSLevel};
    use crate::publish::{
        DupFlag, Publish, RetainFlag, MSG_DUP_FLAG_1_WITH_QOS_LEVEL_0, MSG_INVALID_PACKET_ID,
        MSG_PACKET_TYPE_PUBLISH, MSG_TOPIC_NAME_ONE_CHAR, MSG_TOPIC_WILDCARDS,
    };
    use crate::utf8::Field;
    use std::io::Cursor;
    use std::vec;

    #[test]
    fn test_dup_flag_0_with_qos_level_different_from_0_should_raise_invalid_dup_flag() {
        let control_byte = 0b111000u8;
        let dummy: Vec<u8> = vec![0b111000];
        let mut stream = Cursor::new(dummy);
        let expected_error =
            PacketError::new_kind(MSG_DUP_FLAG_1_WITH_QOS_LEVEL_0, ErrorKind::InvalidDupFlag);
        let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_qos_level_3_should_raise_invalid_qos_level_error() {
        let control_byte = 0b110110u8;
        let dummy: Vec<u8> = vec![0b110110];
        let mut stream = Cursor::new(dummy);
        let expected_error = PacketError::new_kind("Invalid QoS level", ErrorKind::InvalidQoSLevel);
        let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_packet_control_type_5_should_raise_invalid_control_packet_type_error() {
        let control_byte = 0b100000u8;
        let dummy: Vec<u8> = vec![0b100000];
        let mut stream = Cursor::new(dummy);
        let expected_error =
            PacketError::new_kind(MSG_PACKET_TYPE_PUBLISH, ErrorKind::InvalidControlPacketType);
        let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_publish_packet_with_qos_level_0_must_not_have_a_packet_id() {
        let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = Vec::from("mensaje".as_bytes());
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);
        let mut bytes = vec![remaining_data.len() as u8];
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
        let result = Publish::read_from(&mut stream, control_byte).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_publish_packet_with_qos_level_1_must_have_a_packet_id() {
        let control_byte = 0b110010u8; // primer byte con los flags con QoS level 1;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = Vec::from("mensaje".as_bytes());
        let mut packet_id_buf: Vec<u8> = vec![0b0, 0b1010]; // Seria 01010 = packet identifier 10;
        remaining_data.append(&mut topic);
        remaining_data.append(&mut packet_id_buf);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected = Publish {
            packet_id: Option::from(10_u16),
            topic_name: "a/b".to_string(),
            qos: QoSLevel::QoSLevel1,
            retain_flag: RetainFlag::RetainFlag0,
            dup_flag: DupFlag::DupFlag0,
            payload: Option::from("mensaje".to_string()),
        };
        let result = Publish::read_from(&mut stream, control_byte).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_publish_packet_might_have_zero_length_payload() {
        let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = vec![];
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
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
        let result = Publish::read_from(&mut stream, control_byte).unwrap();
        assert_eq!(expected, result);
        assert_eq!(expected.payload().unwrap(), "");
    }

    #[test]
    fn test_publish_packet_topics_should_be_case_sensitive() {
        let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/B").unwrap().encode();
        let mut payload = Vec::from("aa".as_bytes());
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
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
        let result = Publish::read_from(&mut stream, control_byte).unwrap();
        assert_ne!(expected, result);
    }

    #[test]
    fn test_publish_packet_topic_name_must_be_at_least_one_character_long() {
        let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("").unwrap().encode();
        let mut payload = Vec::from("aa".as_bytes());
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected_error = PacketError::new_kind(
            "Topic name must be at least one character long for a Publish packet",
            ErrorKind::TopicNameMustBeAtLeastOneCharacterLong,
        );
        let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(expected_error, result);
    }
    #[test]
    fn test_topic_name_can_not_have_wildcards() {
        let control_byte = 0b110000u8; // primer byte con los flags con QoS level 0;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("asd/#").unwrap().encode();
        let mut payload = "mensaje".as_bytes().to_vec();
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected_error = PacketError::new_kind(
            MSG_TOPIC_WILDCARDS,
            ErrorKind::TopicNameMustNotHaveWildcards,
        );
        let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(expected_error, result);
    }

    #[test]
    fn test_publish_packet_can_not_have_packet_id_0() {
        let control_byte = 0b110010u8; // primer byte con los flags con QoS level 1;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = "los pollos hermanos".as_bytes().to_vec();
        let mut packet_id_buf: Vec<u8> = vec![0b0, 0b0]; // Packet identifier 0
        remaining_data.append(&mut topic);
        remaining_data.append(&mut packet_id_buf);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let expected_error = PacketError::new_msg(MSG_INVALID_PACKET_ID);
        let result = Publish::read_from(&mut stream, control_byte).unwrap_err();
        assert_eq!(expected_error, result);
    }

    #[test]
    fn test_max_qos() {
        let control_byte = 0b110100u8; // primer byte con los flags con QoS level 2;
        let mut remaining_data: Vec<u8> = vec![];
        let mut topic = Field::new_from_string("a/b").unwrap().encode();
        let mut payload = "aa".as_bytes().to_vec();
        remaining_data.append(&mut topic);
        remaining_data.append(&mut payload);

        let mut bytes = vec![remaining_data.len() as u8];
        bytes.append(&mut remaining_data);
        let mut stream = Cursor::new(bytes);
        let mut result = Publish::read_from(&mut stream, control_byte).unwrap();

        assert_eq!(*result.qos(), QoSLevel::QoSLevel2);
        result.set_max_qos(QoSLevel::QoSLevel1);
        assert_eq!(*result.qos(), QoSLevel::QoSLevel1);
        result.set_max_qos(QoSLevel::QoSLevel0);
        assert_eq!(*result.qos(), QoSLevel::QoSLevel0);
    }
}
