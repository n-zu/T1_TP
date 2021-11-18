use std::io::Read;

use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::PacketResult,
    packet_reader,
    traits::MQTTDecoding,
};

use super::*;

impl MQTTDecoding for Suback {
    /// Reads from a stream of bytes and returns a valid Suback packet
    /// It is assumed that the first byte was read into control_byte parameter
    ///
    /// # Errors
    ///
    /// This function returns a PacketError if:
    /// - Control packet type is different from 9
    /// - Reserved bits are not 0b0000
    /// - Remaining length is greater than 256 MB
    /// - Any return code does not match any of these 0x00, 0x01, 0x80
    ///
    /// # Examples
    /// ```
    /// use std::io::Cursor;
    /// use packets::suback::Suback;
    /// use crate::packets::traits::MQTTDecoding;
    ///
    /// let stream_aux = vec![6, 0, 1, 1, 0, 1, 0];
    /// let control_byte = 0b10010000;
    /// let mut stream = Cursor::new(stream_aux);
    /// let result = Suback::read_from(&mut stream, control_byte).unwrap().encode().unwrap();
    /// let expected = vec![control_byte, 6, 0, 1, 1, 0, 1, 0];
    /// assert_eq!(result, expected);
    /// ```
    fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Suback> {
        check_packet_type(control_byte, PacketType::Suback)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(bytes)?;
        let subscribe_packet_id = Self::read_packet_id(&mut remaining_bytes);
        let return_codes = Self::read_return_codes(&mut remaining_bytes)?;
        Self::verify_return_codes_from_vec(&return_codes)?;
        Ok(Self {
            return_codes,
            subscribe_packet_id,
            topics: Vec::new(),
        })
    }
}

impl Suback {
    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut subscribe_packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut subscribe_packet_id_buffer);
        u16::from_be_bytes(subscribe_packet_id_buffer)
    }

    #[doc(hidden)]
    fn read_return_codes(bytes: &mut impl Read) -> PacketResult<Vec<u8>> {
        let mut return_codes = Vec::new();
        bytes.read_to_end(&mut return_codes)?;
        Ok(return_codes)
    }
}
