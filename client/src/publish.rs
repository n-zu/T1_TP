#![allow(dead_code)]

use std::vec;

use packets::{
    packet_reader::{PacketError, RemainingLength},
    utf8::Field,
};

use crate::connect::QoSLevel;

#[doc(hidden)]
const RETAIN_FLAG: u8 = 0b00000001;
#[doc(hidden)]
const QOS_LEVEL_0_FLAG: u8 = 0b00000000;
#[doc(hidden)]
const QOS_LEVEL_1_FLAG: u8 = 0b00000010;
#[doc(hidden)]
const DUP_FLAG: u8 = 0b00001000;
#[doc(hidden)]
const PUBLISH_PACKET_TYPE: u8 = 0b00110000;
#[doc(hidden)]
const SINGLE_LEVEL_WILDCARD: char = '+';
#[doc(hidden)]
const MULTI_LEVEL_WILDCARD: char = '#';

/// A Publish control packet is sent from a Client to a Server
/// or from Server to a Client to transport an Application Message.
pub struct Publish {
    /// Indicates if this is the first occasion that the Client or
    /// the Server has attempted to send this packet, or if this
    /// might be re-delivery of an earlier attempt to send the packet
    duplicated: bool,
    /// Indicates the level of assurance for delivery of an application message
    qos: QoSLevel,
    /// If the retain flag is set to true, in a Publish packet sent by a Client to a Server,
    /// the Server must store the Application Message and its QoS, so that it can be delivered
    /// to future subscribers whose subscriptions match its topic name
    retain: bool,
    /// Identifies the information channel to which payload data is published.
    topic_name: Field,
    /// Contains the Application Message that is being published
    topic_message: Field,
    /// Identifies the packet if the QoS level is 1 or 2
    packet_identifier: Option<u16>,
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
        duplicated: bool,
        qos: QoSLevel,
        retain: bool,
        topic_name: &str,
        topic_message: &str,
        packet_identifier: Option<u16>,
    ) -> Result<Self, PacketError> {
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
            duplicated,
            qos,
            retain,
            topic_name: Field::new_from_string(topic_name)?,
            topic_message: Field::new_from_string(topic_message)?,
            packet_identifier,
        })
    }

    #[doc(hidden)]
    fn check_topic_name_cannot_contain_wildcard_characters(
        topic_name: &str,
    ) -> Result<(), PacketError> {
        if topic_name.contains(SINGLE_LEVEL_WILDCARD) || topic_name.contains(MULTI_LEVEL_WILDCARD) {
            return Err(PacketError::new_msg(
                "El topic name no debe contener wildcards",
            ));
        }
        Ok(())
    }

    pub fn mark_as_duplicated(&mut self) {
        self.duplicated = true
    }

    pub fn duplicated(&self) -> bool {
        self.duplicated
    }

    pub fn qos(&self) -> &QoSLevel {
        &self.qos
    }

    pub fn retain(&self) -> bool {
        self.retain
    }

    pub fn topic_name(&self) -> &str {
        self.topic_name.decode()
    }

    pub fn topic_message(&self) -> &str {
        self.topic_message.decode()
    }

    pub fn packet_identifier(&self) -> Option<u16> {
        self.packet_identifier
    }

    #[doc(hidden)]
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header = vec![];
        variable_header.append(&mut self.topic_name.encode());
        if let Some(packet_identifier) = self.packet_identifier {
            variable_header.push(packet_identifier.to_be_bytes()[0]);
            variable_header.push(packet_identifier.to_be_bytes()[1]);
        }

        variable_header
    }

    #[doc(hidden)]
    fn control_byte(&self) -> u8 {
        let mut control_byte = PUBLISH_PACKET_TYPE;
        if self.duplicated {
            control_byte |= DUP_FLAG;
        }
        match self.qos {
            QoSLevel::QoSLevel0 => control_byte |= QOS_LEVEL_0_FLAG,
            QoSLevel::QoSLevel1 => control_byte |= QOS_LEVEL_1_FLAG,
        }
        if self.retain {
            control_byte |= RETAIN_FLAG;
        }
        control_byte
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Result<Vec<u8>, PacketError> {
        let mut fixed_header = vec![];
        let variable_header_len = self.variable_header().len();
        let message_len = self.topic_message.encode().len();
        let remaining_length = RemainingLength::from_uncoded(variable_header_len + message_len)?;
        let control_byte = self.control_byte();
        fixed_header.push(control_byte);
        fixed_header.append(&mut remaining_length.encode());
        Ok(fixed_header)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PacketError> {
        let mut bytes = vec![];
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut self.topic_message.encode());
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::connect::QoSLevel;

    use super::Publish;

    #[test]
    fn basic_test() {
        let packet =
            Publish::new(false, QoSLevel::QoSLevel0, false, "topic", "message", None).unwrap();
        assert_eq!(
            packet.encode().unwrap(),
            [
                0b00110000, // control_byte
                16,         // remaining_length
                0, 5, // largo topic_name
                116, 111, 112, 105, 99, // topic
                0, 7, // largo topic_message
                109, 101, 115, 115, 97, 103, 101 // message
            ]
        );
    }

    #[test]
    fn test_retain_flag() {
        let packet =
            Publish::new(false, QoSLevel::QoSLevel0, true, "topic", "message", None).unwrap();
        assert_eq!(
            packet.encode().unwrap(),
            [
                0b00110001, // control_byte
                16,         // remaining_length
                0, 5, // largo topic_name
                116, 111, 112, 105, 99, // topic
                0, 7, // largo topic_message
                109, 101, 115, 115, 97, 103, 101 // message
            ]
        );
    }

    #[test]
    fn test_qos_level_1() {
        let packet = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "topic",
            "message",
            Some(153),
        )
        .unwrap();
        assert_eq!(
            packet.encode().unwrap(),
            [
                0b00110010, // control_byte
                18,         // remaining_length
                0, 5, // largo topic_name
                116, 111, 112, 105, 99, // topic
                0, 153, // packet_identifier
                0, 7, // largo topic_message
                109, 101, 115, 115, 97, 103, 101 // message
            ]
        );
    }

    #[test]
    fn test_packet_identifier() {
        let packet = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "topic",
            "message",
            Some(350),
        )
        .unwrap();
        assert_eq!(
            packet.encode().unwrap(),
            [
                0b00110010, // control_byte
                18,         // remaining_length
                0, 5, // largo topic_name
                116, 111, 112, 105, 99, // topic
                1, 94, // packet_identifier
                0, 7, // largo topic_message
                109, 101, 115, 115, 97, 103, 101 // message
            ]
        );
    }

    #[test]
    fn test_publish_cannot_have_packet_identifier_with_qos_0() {
        let packet = Publish::new(
            false,
            QoSLevel::QoSLevel0,
            false,
            "topic",
            "message",
            Some(350),
        );
        assert!(packet.is_err());
    }

    #[test]
    fn test_publish_must_have_packet_identifier_with_qos_1() {
        let packet = Publish::new(false, QoSLevel::QoSLevel1, false, "topic", "message", None);
        assert!(packet.is_err());
    }

    #[test]
    fn test_topic_name_cannot_contain_single_level_wildcard() {
        let packet = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "topic+",
            "message",
            Some(350),
        );
        assert!(packet.is_err());
    }

    #[test]
    fn test_topic_name_cannot_contain_multi_level_wildcard() {
        let packet = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "topic#",
            "message",
            Some(350),
        );
        assert!(packet.is_err());
    }

    #[test]
    fn test_zero_length_payload() {
        let packet =
            Publish::new(false, QoSLevel::QoSLevel1, false, "topic", "", Some(350)).unwrap();
        assert_eq!(
            packet.encode().unwrap(),
            [
                0b00110010, // control_byte
                9,          // remaining_length
                0, 5, // largo topic_name
                116, 111, 112, 105, 99, // topic
                1, 94, // packet_identifier
            ]
        );
    }
}
