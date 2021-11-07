#![allow(unused)]
use packets::packet_reader::{ErrorKind, PacketError, RemainingLength};
use packets::utf8::Field;

#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";
#[doc(hidden)]
const CONTROL_TYPE_UNSUBSCRIBE: u8 = 0b10100010;

#[derive(Debug)]
/// Client-side Unsubscribe packet struct
pub struct Unsubscribe {
    packet_id: u16,
    topic_filters: Vec<String>,
}

impl Unsubscribe {
    /// Returns a new Unsubscribe packet given a packet id and topic filters
    ///
    /// # Errors
    ///
    /// This functions returns PacketError if:
    /// - packet_id is zero
    pub fn new(packet_id: u16, topic_filters: Vec<String>) -> Result<Unsubscribe, PacketError> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self {
            packet_id,
            topic_filters,
        })
    }

    /// Encodes Unsubscribe packet into its byte representation following MQTT v3.1.1 protocol
    ///
    /// # Errors
    ///
    /// This function returns PacketError if:
    /// - Remaining length is greater than 256 MB
    pub fn encode(&self) -> Result<Vec<u8>, PacketError> {
        let mut bytes = Vec::new();
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut self.payload()?);
        Ok(bytes)
    }

    #[doc(hidden)]
    fn verify_packet_id(packet_id: &u16) -> Result<(), PacketError> {
        if *packet_id == 0 {
            return Err(PacketError::new_kind(
                MSG_INVALID_PACKET_ID,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    /// Encodes Unsubscribe fixed header
    fn fixed_header(&self) -> Result<Vec<u8>, PacketError> {
        let mut fixed_header: Vec<u8> = vec![CONTROL_TYPE_UNSUBSCRIBE];
        let remaining_length = RemainingLength::from_uncoded(
            self.payload().unwrap().len() + self.variable_header().len(),
        )?;
        let mut remaining_length_buff = remaining_length.encode();
        fixed_header.append(&mut remaining_length_buff);
        Ok(fixed_header)
    }

    #[doc(hidden)]
    /// Encodes Unsubscribe variable header
    fn variable_header(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let packet_id_representation = self.packet_id.to_be_bytes();
        bytes.append(&mut packet_id_representation.to_vec());
        bytes
    }

    #[doc(hidden)]
    /// Encodes Unsubscribe payload
    fn payload(&self) -> Result<Vec<u8>, PacketError> {
        let mut payload_buffer = Vec::new();
        for topic in &self.topic_filters {
            let field = Field::new_from_string(topic)?;
            let mut field_encoded = field.encode();
            payload_buffer.append(&mut field_encoded);
        }
        Ok(payload_buffer)
    }
}

#[cfg(test)]
mod tests {
    use crate::unsubscribe::{Unsubscribe, CONTROL_TYPE_UNSUBSCRIBE, MSG_INVALID_PACKET_ID};
    use packets::packet_reader::{ErrorKind, PacketError};
    use packets::utf8::Field;

    #[test]
    fn unsubscribe_packet_with_packet_id_0_should_raise_protocol_error() {
        let topics = vec!["temperatura/Argentina".to_string()];
        let result = Unsubscribe::new(0, topics).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_PACKET_ID, ErrorKind::InvalidProtocol);
        assert_eq!(result, expected_error)
    }

    #[test]
    fn valid_unsubscribe_packet_with_id_1_and_one_topic() {
        let topics = vec!["temperatura/Argentina".to_string()];
        let result = Unsubscribe::new(1, topics).unwrap().encode().unwrap();
        let mut topic_encoded = Field::new_from_string("temperatura/Argentina")
            .unwrap()
            .encode();
        let mut expected: Vec<u8> = vec![
            CONTROL_TYPE_UNSUBSCRIBE,
            25, // remaining length
            0,
            1, // packet id
        ];
        expected.append(&mut topic_encoded);
        assert_eq!(result, expected);
    }

    #[test]
    fn valid_unsubscribe_packet_with_id_5_and_two_topic() {
        let topics = vec![
            "temperatura/Argentina".to_string(),
            "temperatura/Uruguay".to_string(),
        ];
        let result = Unsubscribe::new(5, topics).unwrap().encode().unwrap();
        let mut topic_encoded_arg = Field::new_from_string("temperatura/Argentina")
            .unwrap()
            .encode();
        let mut topic_encoded_uru = Field::new_from_string("temperatura/Uruguay")
            .unwrap()
            .encode();
        let mut expected: Vec<u8> = vec![
            CONTROL_TYPE_UNSUBSCRIBE,
            46, // remaining length
            0,
            5, // packet id
        ];
        expected.append(&mut topic_encoded_arg);
        expected.append(&mut topic_encoded_uru);
        assert_eq!(result, expected);
    }

    #[test]
    fn unsubscribe_packet_can_have_empty_string_as_topic_filter() {
        let topics = vec!["".to_string()];
        let result = Unsubscribe::new(1, topics).unwrap().encode().unwrap();
        let mut topic_encoded = Field::new_from_string("").unwrap().encode();
        let mut expected: Vec<u8> = vec![
            CONTROL_TYPE_UNSUBSCRIBE,
            4, // remaining length
            0,
            1, // packet id
        ];
        expected.append(&mut topic_encoded);
        assert_eq!(result, expected);
    }

    #[test]
    fn client_unsubscribe_packet_can_have_empty_topic_filter() {
        let topics = vec![];
        let result = Unsubscribe::new(1, topics).unwrap().encode().unwrap();
        let expected: Vec<u8> = vec![
            CONTROL_TYPE_UNSUBSCRIBE,
            2, // remaining length
            0,
            1, // packet id
        ];
        assert_eq!(result, expected);
    }
}
