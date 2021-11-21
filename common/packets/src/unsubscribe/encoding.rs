#![allow(unused)]

use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    packet_reader::RemainingLength,
    traits::{MQTTBytes, MQTTEncoding},
    utf8::Field,
};

use super::*;

impl MQTTEncoding for Unsubscribe {
    /// Encodes Unsubscribe packet into its byte representation following MQTT v3.1.1 protocol
    ///
    /// # Errors
    ///
    /// This function returns PacketError if:
    /// - Remaining length is greater than 256 MB
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let mut bytes = Vec::new();
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut self.payload()?);
        Ok(bytes)
    }
}

impl Unsubscribe {
    /// Returns a new Unsubscribe packet given a packet id and topic filters
    ///
    /// # Errors
    ///
    /// This functions returns PacketError if:
    /// - packet_id is zero
    pub fn new(packet_id: u16, topic_filters: Vec<String>) -> PacketResult<Unsubscribe> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self {
            packet_id,
            topic_filters,
        })
    }

    #[doc(hidden)]
    fn verify_packet_id(packet_id: &u16) -> PacketResult<()> {
        if *packet_id == 0 {
            return Err(PacketError::new_kind(
                MSG_INVALID_PACKET_ID,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    // Encodes Unsubscribe fixed header
    fn fixed_header(&self) -> PacketResult<Vec<u8>> {
        let control_byte = build_control_byte(PacketType::Unsubscribe, RESERVED_BITS);
        let mut fixed_header: Vec<u8> = vec![control_byte];
        let remaining_length = RemainingLength::from_uncoded(
            self.payload().unwrap().len() + self.variable_header().len(),
        )?;
        let mut remaining_length_buff = remaining_length.encode();
        fixed_header.append(&mut remaining_length_buff);
        Ok(fixed_header)
    }

    #[doc(hidden)]
    // Encodes Unsubscribe variable header
    fn variable_header(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let packet_id_representation = self.packet_id.to_be_bytes();
        bytes.append(&mut packet_id_representation.to_vec());
        bytes
    }

    #[doc(hidden)]
    // Encodes Unsubscribe payload
    fn payload(&self) -> PacketResult<Vec<u8>> {
        let mut payload_buffer = Vec::new();
        for topic in &self.topic_filters {
            let field = Field::new_from_string(topic)?;
            let mut field_encoded = field.encode();
            payload_buffer.append(&mut field_encoded);
        }
        Ok(payload_buffer)
    }
}
