use std::io::Read;

use crate::qos::QoSLevel;
use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
    utf8::Field,
};

use super::*;

impl MQTTDecoding for Unsubscribe {
    /// Reads from a stream of bytes and returns a valid Unsubscribe packet
    /// It is assumed that the first byte was read into control_byte parameter
    ///
    ///
    /// # Errors
    ///
    /// This function returns a PacketError if:
    /// - Control packet type is different from 10
    /// - Reserved bits are not 0b0010
    /// - Remaining length is greater than 256 MB
    /// - Topic filter is empty
    fn read_from<T: Read>(stream: &mut T, control_byte: u8) -> PacketResult<Unsubscribe> {
        check_packet_type(control_byte, PacketType::Unsubscribe)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(stream)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        let mut topic_filters: Vec<TopicFilter> = Vec::new();
        Self::read_topic_filters(&mut remaining_bytes, &mut topic_filters)?;
        Ok(Unsubscribe {
            packet_id,
            topic_filters,
        })
    }
}

impl Unsubscribe {
    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }

    #[doc(hidden)]
    fn read_topic_filters(
        bytes: &mut impl Read,
        topic_filters_buffer: &mut Vec<TopicFilter>,
    ) -> PacketResult<()> {
        while let Some(topic_filter) = Field::new_from_stream(bytes) {
            Self::verify_at_least_one_character_long_topic_filter(&topic_filter)?;
            topic_filters_buffer.push(TopicFilter::new(&topic_filter.value, QoSLevel::QoSLevel0)?);
        }

        if topic_filters_buffer.is_empty() {
            return Err(PacketError::new_kind(
                MSG_AT_LEAST_ONE_TOPIC_FILTER,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_at_least_one_character_long_topic_filter(topic_filter: &Field) -> PacketResult<()> {
        if topic_filter.is_empty() {
            return Err(PacketError::new_kind(
                MSG_AT_LEAST_ONE_CHAR_LONG_TOPIC_FILTER,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }
}
