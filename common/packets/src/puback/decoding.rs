use std::io::{self, Read};

use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{PacketError, PacketResult},
    packet_reader,
    traits::MQTTDecoding,
};

use super::*;

impl MQTTDecoding for Puback {
    /// Creates a new Puback packet from a given stream of bytes.
    ///
    /// It is assumed the first byte from the stream was read by the client/server
    ///
    /// # Errors
    ///
    /// If reserved bits from the bytes stream doesn't follow MQTT 3.1.1 (this is 0b0), this function returns an invalid protocol error
    ///
    /// If control packet type bits from the bytes stream doesn't follow MQTT 3.1.1 (this is 0b01000000), this function returns an invalid control packet type error
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    /// use packets::puback::Puback;
    /// use crate::packets::traits::MQTTDecoding;
    /// let control_byte = 0b01000000u8;
    /// let remaining_length = 2u8;
    /// let data_buffer: Vec<u8> = vec![remaining_length, 0, 1];
    /// let mut stream = Cursor::new(data_buffer);
    /// let expected = Puback::new(1).unwrap();
    /// let result = Puback::read_from(&mut stream, control_byte).unwrap();
    /// assert_eq!(expected, result);
    /// ```
    fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Self> {
        check_packet_type(control_byte, PacketType::Puback)?;
        check_reserved_bits(control_byte, RESERVED_BITS)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(bytes)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        Self::verify_packet_end(&mut remaining_bytes)?;
        Ok(Self { packet_id })
    }
}

impl Puback {
    #[doc(hidden)]
    fn read_packet_id(bytes: &mut impl Read) -> u16 {
        let mut packet_id_buffer = [0u8; 2];
        let _ = bytes.read_exact(&mut packet_id_buffer);
        u16::from_be_bytes(packet_id_buffer)
    }

    #[doc(hidden)]
    fn verify_packet_end(bytes: &mut impl Read) -> PacketResult<()> {
        let mut buff = [0u8; 1];
        match bytes.read_exact(&mut buff) {
            Ok(()) => Err(PacketError::new_msg(MSG_PACKET_MORE_BYTES_THAN_EXPECTED)),
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
            Err(_) => Err(PacketError::new_msg("Error at reading packet")),
        }
    }
}
