#![allow(dead_code)]

use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::{PacketError, PacketResult},
    packet_reader::RemainingLength,
    traits::{MQTTBytes, MQTTEncoding},
    utf8::Field,
};

use super::*;

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

impl MQTTEncoding for Publish {
    ///
    /// Encode a Publish packet into a vector of bytes
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * packet_identifier is None and QoS is 1
    /// * packet_identifier is not None and QoS is 0
    /// * topic_name or topic_message exceedes the maximum length
    ///   established for UTF-8 fields in MQTT V3.1.1 standard
    /// * topic_name contains wildcard characters
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let mut bytes = vec![];
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut Vec::from(self.payload.as_ref().unwrap().as_bytes()));
        Ok(bytes)
    }
}

impl Publish {
    /// Creates a new Publish packet
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * packet_identifier is None and QoS is 1
    /// * packet_identifier is not None and QoS is 0
    /// * topic_name or topic_message exceedes the maximum length
    ///   established for uft8 fields in MQTT V3.1.1 standard
    /// * topic_name contains wildcard characters
    pub fn new(
        dup_flag: bool,
        qos: QoSLevel,
        retain_flag: bool,
        topic_name: &str,
        topic_message: &str,
        packet_identifier: Option<u16>,
    ) -> PacketResult<Self> {
        if packet_identifier.is_some() && qos == QoSLevel::QoSLevel0 {
            return Err(PacketError::new_msg(
                "Un paquete con QoS 0 no puede tener identificador",
            ));
        } else if packet_identifier.is_none() && qos == QoSLevel::QoSLevel1 {
            return Err(PacketError::new_msg(
                "Un paquete con QoS 1 debe tener un identificador",
            ));
        }

        Publish::check_topic_name_cannot_contain_wildcard_characters(topic_name)?;

        Ok(Self {
            packet_id: packet_identifier,
            topic_name: topic_name.to_string(),
            qos,
            retain_flag,
            dup_flag,
            payload: Some(topic_message.to_string()),
        })
    }

    #[doc(hidden)]
    fn check_topic_name_cannot_contain_wildcard_characters(topic_name: &str) -> PacketResult<()> {
        if topic_name.contains(SINGLE_LEVEL_WILDCARD) || topic_name.contains(MULTI_LEVEL_WILDCARD) {
            return Err(PacketError::new_msg(
                "El topic name no debe contener wildcards",
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn variable_header(&self) -> MQTTBytes {
        let mut variable_header = vec![];
        variable_header.append(&mut Field::new_from_string(&self.topic_name).unwrap().encode());
        if let Some(packet_identifier) = self.packet_id {
            if self.qos != QoSLevel::QoSLevel0 {
                variable_header.push(packet_identifier.to_be_bytes()[0]);
                variable_header.push(packet_identifier.to_be_bytes()[1]);
            }
        }

        variable_header
    }

    #[doc(hidden)]
    fn reserved_bits(&self) -> u8 {
        let mut bits = 0;
        if self.dup_flag {
            bits |= DUP_FLAG;
        }
        if self.retain_flag {
            bits |= RETAIN_FLAG;
        }
        bits |= (self.qos() as u8) << QOS_SHIFT;
        bits
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> PacketResult<MQTTBytes> {
        let mut fixed_header = vec![];
        let variable_header_len = self.variable_header().len();
        let message_len = self.payload.as_ref().unwrap().as_bytes().len();
        let remaining_length = RemainingLength::from_uncoded(variable_header_len + message_len)?;
        let control_byte = build_control_byte(PacketType::Publish, self.reserved_bits());
        fixed_header.push(control_byte);
        fixed_header.append(&mut remaining_length.encode());
        Ok(fixed_header)
    }
}
