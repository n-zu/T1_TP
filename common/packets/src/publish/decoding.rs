use std::{convert::TryInto, io::Read};

use crate::{
    helpers::{check_packet_type, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
    utf8::Field,
};

use super::*;

impl MQTTDecoding for Publish {
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
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    /// use packets::publish::Publish;
    /// use packets::qos::QoSLevel;
    /// use packets::utf8::Field;
    /// use packets::traits::MQTTDecoding;
    ///
    ///  let control_byte = 0b110010; // primer byte con los flags con QoS level 1;
    ///  let mut remaining_data: Vec<u8> = vec![];
    ///  let mut topic = Field::new_from_string("a/b").unwrap().encode();    ///
    ///  let mut payload = "mensaje".as_bytes().to_vec();
    ///  let mut packet_id_buf: Vec<u8> = vec![0b0, 0b1010]; // Seria 01010 = packet identifier 10;
    ///  remaining_data.append(&mut topic);
    ///  remaining_data.append(&mut packet_id_buf);
    ///  remaining_data.append(&mut payload);
    ///  let mut bytes = vec![];
    ///  bytes.push(remaining_data.len() as u8);
    ///  bytes.append(&mut remaining_data);
    ///  let mut stream = Cursor::new(bytes);
    ///  let expected = Publish::new(
    ///     false,
    ///     QoSLevel::QoSLevel1,
    ///     false,
    ///     "a/b",
    ///     "mensaje",
    ///     Some(10),
    ///  ).unwrap();
    ///  let result = Publish::read_from(&mut stream, control_byte).unwrap();
    ///  assert_eq!(expected, result);
    /// ```
    fn read_from<T: Read>(stream: &mut T, control_byte: u8) -> PacketResult<Publish> {
        let retain_flag = Self::verify_retain_flag(&control_byte);
        let qos_level = Self::verify_qos_level_flag(&control_byte)?;
        let dup_flag = Self::verify_dup_flag(&control_byte, qos_level)?;
        check_packet_type(control_byte, PacketType::Publish)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(stream)?;
        let topic_name = Self::verify_topic_name(&mut remaining_bytes)?;
        let packet_id = Self::verify_packet_id(&mut remaining_bytes, &qos_level)?;
        let payload = Self::read_payload(&mut remaining_bytes)?;
        Ok(Self {
            packet_id,
            topic_name: topic_name.value,
            qos: qos_level,
            retain_flag,
            dup_flag,
            payload,
        })
    }
}

impl Publish {
    #[doc(hidden)]
    fn read_payload(bytes: &mut impl Read) -> PacketResult<String> {
        let mut payload_buf = vec![];
        let _ = bytes.read_to_end(&mut payload_buf);
        Ok(String::from_utf8(payload_buf)?)
    }

    #[doc(hidden)]
    fn verify_retain_flag(control_byte: &u8) -> bool {
        let retain_flag = control_byte & 0b1;
        retain_flag == 1
    }

    #[doc(hidden)]
    fn verify_qos_level_flag(control_byte: &u8) -> PacketResult<QoSLevel> {
        let qos_level = (control_byte & 0b110) >> 1;
        qos_level.try_into()
    }

    #[doc(hidden)]
    fn verify_dup_flag(control_byte: &u8, qos_level: QoSLevel) -> PacketResult<bool> {
        let dup_flag = (control_byte & 0b1000) >> 3;
        if qos_level == QoSLevel::QoSLevel0 && dup_flag == 1 {
            return Err(PacketError::new_kind(
                MSG_DUP_FLAG_1_WITH_QOS_LEVEL_0,
                ErrorKind::InvalidDupFlag,
            ));
        }
        Ok(dup_flag == 1)
    }

    #[doc(hidden)]
    fn verify_packet_id(bytes: &mut impl Read, qos_level: &QoSLevel) -> PacketResult<Option<u16>> {
        if *qos_level == QoSLevel::QoSLevel0 {
            return Ok(None);
        }
        let mut packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        let packet_id = u16::from_be_bytes(packet_id_buffer);
        if packet_id == 0 {
            return Err(PacketError::new_msg(MSG_INVALID_PACKET_ID));
        }
        Ok(Some(packet_id))
    }

    #[doc(hidden)]
    fn verify_topic_name(bytes: &mut impl Read) -> PacketResult<Field> {
        let topic_name = Field::new_from_stream(bytes).ok_or_else(PacketError::new)?;
        let topic_name_chars_count = topic_name.value.chars().count();
        if topic_name_chars_count == 0 {
            return Err(PacketError::new_kind(
                MSG_TOPIC_NAME_ONE_CHAR,
                ErrorKind::TopicNameMustBeAtLeastOneCharacterLong,
            ));
        }
        if topic_name.value.contains(SINGLE_LEVEL_WILDCARD)
            || topic_name.value.contains(MULTI_LEVEL_WILDCARD)
        {
            return Err(PacketError::new_kind(
                MSG_TOPIC_WILDCARDS,
                ErrorKind::TopicNameMustNotHaveWildcards,
            ));
        }
        Ok(topic_name)
    }
}
