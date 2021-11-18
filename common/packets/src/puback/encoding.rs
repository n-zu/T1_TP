use crate::{
    packet_error::{ErrorKind, PacketError, PacketResult},
    traits::{MQTTBytes, MQTTEncoding},
};

use super::*;

impl MQTTEncoding for Puback {
    /// Encodes a Puback packet into its bytes representation following MQTT 3.1.1 protocol
    ///
    /// # Examples
    /// ```
    /// use packets::puback::Puback;
    ///
    /// let puback = Puback::new(1).unwrap();
    /// let result = puback.encode();
    /// let expected: Vec<u8> = vec![0b01000000, 0b10, 0b0, 0b1];
    /// assert_eq!(expected, result)
    /// ```
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let mut bytes: Vec<u8> = vec![];
        bytes.append(&mut self.fixed_header());
        bytes.append(&mut self.variable_header());
        Ok(bytes)
    }
}

impl Puback {
    /// Returns a new Puback packet with given packet_id
    ///
    /// # Errors
    ///
    /// If packet_id is zero, this functions returns a [ErrorKind::InvalidProtocol]
    pub fn new(packet_id: u16) -> PacketResult<Self> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self { packet_id })
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

    /// Encodes a Puback packet into its bytes representation following MQTT 3.1.1 protocol
    ///
    /// # Examples
    /// ```
    /// use packets::puback::Puback;
    ///
    /// let puback = Puback::new(1).unwrap();
    /// let result = puback.encode();
    /// let expected: Vec<u8> = vec![0b01000000, 0b10, 0b0, 0b1];
    /// assert_eq!(expected, result)
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        bytes.append(&mut self.fixed_header());
        bytes.append(&mut self.variable_header());
        bytes
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> Vec<u8> {
        let fixed_header_buffer: Vec<u8> = vec![0b01000000, 0b10];
        fixed_header_buffer
    }

    #[doc(hidden)]
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header_buffer: Vec<u8> = vec![];
        let packet_id_representation = self.packet_id.to_be_bytes();
        variable_header_buffer.push(packet_id_representation[0]);
        variable_header_buffer.push(packet_id_representation[1]);
        variable_header_buffer
    }
}
