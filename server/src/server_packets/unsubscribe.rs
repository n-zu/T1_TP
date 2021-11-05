use packets::packet_reader;
use packets::packet_reader::{ErrorKind, PacketError};
use packets::utf8::Field;
use std::io::Read;

#[doc(hidden)]
const UNSUBSCRIBE_CONTROL_PACKET_TYPE: u8 = 10;
#[doc(hidden)]
const MSG_PACKET_TYPE_UNSUBSCRIBE: &str = "Packet type must be 10 for a Unsubscribe packet";
#[doc(hidden)]
const FIXED_RESERVED_BITS: u8 = 0b10;
#[doc(hidden)]
const MSG_INVALID_RESERVED_BITS: &str = "Reserved bits are not equal to 2";
#[doc(hidden)]
const MSG_AT_LEAST_ONE_TOPIC_FILTER: &str =
    "Unsubscribe packet must contain at least one topic filter";

#[derive(Debug)]
pub struct Unsubscribe {
    packet_id: u16,
    topic_filters: Vec<String>,
}

impl Unsubscribe {
    pub fn read_from(bytes: &mut impl Read, control_byte: u8) -> Result<Unsubscribe, PacketError> {
        Self::verify_control_packet_type(&control_byte)?;
        Self::verify_reserved_bits(&control_byte)?;
        let mut remaining_bytes = packet_reader::read_packet_bytes(bytes)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        let mut topic_filters: Vec<String> = Vec::new();
        Self::read_topic_filters(&mut remaining_bytes, &mut topic_filters)?;
        Ok(Unsubscribe {
            packet_id,
            topic_filters,
        })
    }

    #[doc(hidden)]
    fn verify_control_packet_type(control_byte: &u8) -> Result<(), PacketError> {
        let control_packet_type = (control_byte & 0b11110000) >> 4;
        if control_packet_type != UNSUBSCRIBE_CONTROL_PACKET_TYPE {
            return Err(PacketError::new_kind(
                MSG_PACKET_TYPE_UNSUBSCRIBE,
                ErrorKind::InvalidControlPacketType,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_reserved_bits(control_byte: &u8) -> Result<(), PacketError> {
        let reserved_bits = control_byte & 0b1111;
        if reserved_bits != FIXED_RESERVED_BITS {
            return Err(PacketError::new_kind(
                MSG_INVALID_RESERVED_BITS,
                ErrorKind::InvalidReservedBits,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }

    #[doc(hidden)]
    fn read_topic_filters(
        bytes: &mut impl Read,
        topic_filters_buffer: &mut Vec<String>,
    ) -> Result<(), PacketError> {
        while let Some(field) = Field::new_from_stream(bytes) {
            topic_filters_buffer.push(field.value);
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
    /// Gets packet id from current Unsubscribe packet
    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }
    #[doc(hidden)]
    /// Gets topic filters from current Unsubscribe packet
    pub fn topic_filters(&self) -> &Vec<String> {
        &self.topic_filters
    }
}

#[cfg(test)]
mod tests {
    use crate::server_packets::unsubscribe::{
        Unsubscribe, MSG_AT_LEAST_ONE_TOPIC_FILTER, MSG_INVALID_RESERVED_BITS,
        MSG_PACKET_TYPE_UNSUBSCRIBE,
    };
    use packets::packet_reader::{ErrorKind, PacketError};
    use packets::utf8::Field;
    use std::io::Cursor;

    #[test]
    fn test_unsubscribe_packet_with_empty_topic_filter_should_raise_invalid_protocol_error() {
        let control_byte = 0b10100010u8;
        let v: Vec<u8> = vec![2, 0, 1]; // remaining length + packet id
        let mut stream = Cursor::new(v);
        let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_AT_LEAST_ONE_TOPIC_FILTER, ErrorKind::InvalidProtocol);
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_unsubscribe_packet_with_control_byte_other_than_10_should_raise_invalid_control_packet_type_error(
    ) {
        let control_byte = 0b10000010u8; // control byte 8 + 0010 reserved bits
        let mut topic = Field::new_from_string("temperatura/uruguay")
            .unwrap()
            .encode();
        let mut v: Vec<u8> = vec![23, 0, 1]; // remaining length + packet id
        v.append(&mut topic); // + payload
        let mut stream = Cursor::new(v);
        let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
        let expected_error = PacketError::new_kind(
            MSG_PACKET_TYPE_UNSUBSCRIBE,
            ErrorKind::InvalidControlPacketType,
        );
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_unsubscribe_packet_with_reserved_bits_other_than_2_should_raise_error() {
        let control_byte = 0b10100000u8; // control byte 10 + reserved bits 0000
        let mut topic = Field::new_from_string("temperatura/uruguay")
            .unwrap()
            .encode();
        let mut v: Vec<u8> = vec![23, 0, 1]; // remaining length + packet id
        v.append(&mut topic); // + payload
        let mut stream = Cursor::new(v);
        let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap_err();
        let expected_error =
            PacketError::new_kind(MSG_INVALID_RESERVED_BITS, ErrorKind::InvalidReservedBits);
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_valid_unsubscribe_packet_with_one_topic() {
        let control_byte = 0b10100010u8; // control byte 10 + reserved bits 0010
        let mut topic = Field::new_from_string("temperatura/uruguay")
            .unwrap()
            .encode();
        let mut v: Vec<u8> = vec![23, 0, 1]; // remaining length + packet id
        v.append(&mut topic); // + payload
        let mut stream = Cursor::new(v);
        let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap();
        assert_eq!(result.packet_id(), 1u16);
        assert_eq!(
            *result.topic_filters(),
            vec!["temperatura/uruguay".to_string()]
        );
    }

    #[test]
    fn test_valid_unsubscribe_packet_with_two_topics() {
        let control_byte = 0b10100010u8; // control byte 10 + reserved bits 0010
        let mut topic_uruguay = Field::new_from_string("temperatura/uruguay")
            .unwrap()
            .encode();
        let mut topic_argentina = Field::new_from_string("temperatura/argentina")
            .unwrap()
            .encode();
        let mut v: Vec<u8> = vec![46, 0, 1]; // remaining length + packet id
        v.append(&mut topic_uruguay); // + payload
        v.append(&mut topic_argentina); // + payload
        let mut stream = Cursor::new(v);
        let result = Unsubscribe::read_from(&mut stream, control_byte).unwrap();
        let expected_id = 1u16;
        let expected_topic_filters = vec![
            "temperatura/uruguay".to_string(),
            "temperatura/argentina".to_string(),
        ];
        assert_eq!(result.packet_id(), expected_id);
        assert_eq!(*result.topic_filters(), expected_topic_filters);
    }
}
