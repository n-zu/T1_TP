#![allow(dead_code)]

use std::{convert::TryFrom, io::Read};

use packets::suback::Suback;
use packets::{
    packet_reader::{self, ErrorKind, PacketError, QoSLevel},
    utf8::Field,
};

#[derive(Debug)]
pub struct TopicFilter {
    /// Topic filter for a subscribe packet
    pub topic_name: String,
    pub qos: QoSLevel,
}

#[derive(Debug)]
pub struct Subscribe {
    /// Server-side subscribe packet structure
    identifier: u16,
    topic_filters: Vec<TopicFilter>,
}

impl Subscribe {
    /// Gets the next two bytes of the stream as an unsigned 16-bit integer.
    /// Returns a PacketError in case they can't be read.
    fn get_identifier(stream: &mut impl Read) -> Result<u16, PacketError> {
        let mut buf = [0; 2];
        stream.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    /// Verifies that the first byte has the correct reserved flags (0010)
    fn verify_reserved_bits(first_byte: &[u8; 1]) -> Result<(), PacketError> {
        if first_byte[0] & 0xF != 2 {
            return Err(PacketError::new());
        }
        Ok(())
    }

    /// Gets the QoSLevel of a topic filter
    fn get_qos(byte: u8) -> Result<QoSLevel, PacketError> {
        QoSLevel::try_from(byte)
    }

    /// Creates a new Subscribe packet from the given stream.
    /// Returns a PacketError in case the packet is malformed.
    /// It is assumed that the first identifier byte has already been read.
    pub fn new(
        stream: &mut impl Read,
        first_byte_buffer: &[u8; 1],
    ) -> Result<Subscribe, PacketError> {
        Self::verify_reserved_bits(first_byte_buffer)?;
        let mut bytes = packet_reader::read_packet_bytes(stream)?;

        let identifier = Self::get_identifier(&mut bytes)?;
        let mut topic_filters = Vec::new();

        while let Some(field) = Field::new_from_stream(&mut bytes) {
            let mut qos_buf = [0; 1];
            bytes.read_exact(&mut qos_buf)?;

            topic_filters.push(TopicFilter {
                topic_name: field.value,
                qos: Self::get_qos(qos_buf[0])?,
            });
        }

        if topic_filters.is_empty() {
            return Err(PacketError::new_kind(
                "No topic filters found",
                ErrorKind::InvalidProtocol,
            ));
        }

        Ok(Subscribe {
            identifier,
            topic_filters,
        })
    }

    /// Creates a response packet (Suback in this case) for this Subscribe packet
    ///
    /// # Errors
    ///
    /// Allowed return codes are 0x00, 0x01, 0x80. If a return code doesn't match any of those, this function returns a [ErrorKind::InvalidReturnCode]
    pub fn response(&self) -> Result<Suback, PacketError> {
        let mut return_codes = Vec::new();
        for topic in &self.topic_filters {
            return_codes.push(topic.qos as u8);
        }
        Suback::new_from_vec(return_codes, self.identifier)
    }

    /// Get the subscribe's identifier.
    pub fn identifier(&self) -> u16 {
        self.identifier
    }

    /// Get a reference to the subscribe's topic filters.
    pub fn topic_filters(&self) -> &[TopicFilter] {
        self.topic_filters.as_ref()
    }

    #[doc(hidden)]
    /// Sets max QoS for each Topic Filter in a Subscribe packet
    /// This is intended to be used by the server in case some QoS is not yet implemented by it
    fn set_max_qos(&mut self, max_qos: QoSLevel) {
        for topic_filter in self.topic_filters.iter_mut() {
            if (topic_filter.qos as u8) > (max_qos as u8) {
                topic_filter.qos = max_qos;
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use packets::packet_reader::QoSLevel;

    use super::Field;
    use super::Subscribe;
    use std::io::Cursor;

    const FIRST_BYTE: [u8; 1] = [0b10000010];

    #[test]
    fn test_identifier() {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[123, 5]); // identifier
        v.extend(Field::new_from_string("unTopic").unwrap().encode());
        v.push(1); // QoS level 1

        v.insert(0, v.len() as u8);
        let packet = Subscribe::new(&mut Cursor::new(v), &FIRST_BYTE).unwrap();
        assert_eq!(packet.identifier(), (123 << 8) + 5);
    }

    #[test]
    fn test_no_topics_should_fail() {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[123, 5]); // identifier

        v.insert(0, v.len() as u8);
        let packet = Subscribe::new(&mut Cursor::new(v), &FIRST_BYTE);
        assert!(packet.is_err());
    }

    #[test]
    fn test_one_topic() {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[123, 5]); // identifier
        v.extend(
            Field::new_from_string("unTopic/*/+/asd//x")
                .unwrap()
                .encode(),
        );
        v.push(1); // QoS level 1

        v.insert(0, v.len() as u8);
        let packet = Subscribe::new(&mut Cursor::new(v), &FIRST_BYTE).unwrap();
        assert_eq!(packet.topic_filters().len(), 1);
        assert_eq!(
            packet.topic_filters().first().unwrap().topic_name,
            "unTopic/*/+/asd//x"
        );
        assert_eq!(
            packet.topic_filters().first().unwrap().qos,
            QoSLevel::QoSLevel1
        );
    }

    #[test]
    fn test_two_topics() {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[123, 5]); // identifier
        v.extend(Field::new_from_string("first").unwrap().encode());
        v.push(2); // QoS level 2
        v.extend(Field::new_from_string("second").unwrap().encode());
        v.push(0); // QoS level 0

        v.insert(0, v.len() as u8);
        let packet = Subscribe::new(&mut Cursor::new(v), &FIRST_BYTE).unwrap();
        assert_eq!(packet.topic_filters().len(), 2);
        assert_eq!(packet.topic_filters().first().unwrap().topic_name, "first");
        assert_eq!(
            packet.topic_filters().first().unwrap().qos,
            QoSLevel::QoSLevel2
        );
        assert_eq!(packet.topic_filters()[1].topic_name, "second");
        assert_eq!(packet.topic_filters()[1].qos, QoSLevel::QoSLevel0);
    }

    #[test]
    fn test_invalid_reserved_flags() {
        let invalid_first = [0b01000011];
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[0, 5]); // identifier
        v.extend(Field::new_from_string("unTopic").unwrap().encode());
        v.push(1); // QoS level 1

        v.insert(0, v.len() as u8);
        let packet = Subscribe::new(&mut Cursor::new(v), &invalid_first);
        assert!(packet.is_err());
    }

    #[test]
    fn test_invalid_qos() {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[0, 5]); // identifier
        v.extend(Field::new_from_string("unTopic").unwrap().encode());
        v.push(3); // QoS level 3

        v.insert(0, v.len() as u8);
        let packet = Subscribe::new(&mut Cursor::new(v), &FIRST_BYTE);
        assert!(packet.is_err());
    }
}
