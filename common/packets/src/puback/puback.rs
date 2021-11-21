#![allow(unused)]

use crate::packet_error::{ErrorKind, PacketError, PacketResult};
use crate::packet_reader;
use std::io;
use std::io::Read;


impl Puback {
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
    /// let control_byte = 0b01000000u8;
    /// let remaining_length = 2u8;
    /// let data_buffer: Vec<u8> = vec![remaining_length, 0, 1];
    /// let mut stream = Cursor::new(data_buffer);
    /// let expected = Puback::new(1).unwrap();
    /// let result = Puback::read_from(&mut stream, control_byte).unwrap();
    /// assert_eq!(expected, result);
    /// ```
    pub fn read_from(bytes: &mut impl Read, control_byte: u8) -> PacketResult<Self> {
        Self::verify_reserved_bits(&control_byte)?;
        Self::verify_control_packet_type(&control_byte)?;
        let mut remaining_bytes = packet_reader::read_remaining_bytes(bytes)?;
        let packet_id = Self::read_packet_id(&mut remaining_bytes);
        Self::verify_packet_end(&mut remaining_bytes)?;
        Ok(Self { packet_id })
    }
}