use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::PacketResult,
    packet_reader::RemainingLength,
    traits::{MQTTBytes, MQTTEncoding},
};

use super::*;

impl MQTTEncoding for Suback {
    /// Returns this Suback packet representation following MQTT v3.1.1 protocol
    /// # Errors
    ///
    /// If remaining length of this packet is greater than 256 MB, this function returns a PacketError
    ///
    /// # Examples
    ///
    /// ```
    ///  use packets::suback::Suback;
    ///  use packets::traits::MQTTEncoding;
    ///
    ///  let return_codes: Vec<u8> = vec![0, 1, 1, 1];
    ///  let mut suback = Suback::new_from_vec(return_codes, 1).unwrap();
    ///  let encoded_suback = suback.encode().unwrap();
    ///  let expected: Vec<u8> = vec![0b10010000, 6, 0, 1, 0, 1, 1, 1];
    ///  assert_eq!(encoded_suback, expected)
    /// ```
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let mut bytes = vec![];
        bytes.append(&mut self.fixed_header()?);
        bytes.append(&mut self.variable_header());
        bytes.append(&mut self.return_codes.clone());
        Ok(bytes)
    }
}
impl Suback {
    /// Returns a new Suback packet struct from a given subscribe packet id and given return codes
    /// The subscribe_packet_id should be the same from the subscribe packet this Suback is acknowledging
    /// The order of return codes in the Suback packet must match the order of Topic Filters in the Subscribe Packet
    ///
    /// # Errors
    ///
    /// Allowed return codes are 0x00, 0x01, 0x80. If a return code doesn't match any of those, this function returns a [ErrorKind::InvalidReturnCode]
    pub fn new_from_vec(return_codes: Vec<u8>, subscribe_packet_id: u16) -> PacketResult<Self> {
        Self::verify_return_codes_from_vec(&return_codes)?;
        Ok(Self {
            return_codes,
            subscribe_packet_id,
            topics: Vec::new(),
        })
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> PacketResult<MQTTBytes> {
        let control_byte = build_control_byte(PacketType::Suback, RESERVED_BITS);
        let mut fixed_header: Vec<u8> = vec![control_byte];
        let remaining_length =
            RemainingLength::from_uncoded(self.return_codes.len() + self.variable_header().len())?;
        let mut remaining_length_buff = remaining_length.encode();
        fixed_header.append(&mut remaining_length_buff);
        Ok(fixed_header)
    }

    #[doc(hidden)]
    fn variable_header(&self) -> MQTTBytes {
        let mut variable_header: Vec<u8> = vec![];
        variable_header.append(&mut self.subscribe_packet_id.to_be_bytes().to_vec());
        variable_header
    }
}
