use packets::packet_reader::{ErrorKind, PacketError};

#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";
const CONTROL_BYTE: u8 = 0b10110000;
const FIXED_REMAINING_LENGTH: u8 = 0b10;

pub struct Unsuback {
    packet_id: u16,
}

impl Unsuback {
    pub fn new(packet_id: u16) -> Result<Unsuback, PacketError> {
        Self::verify_packet_id(&packet_id)?;
        Ok(Self { packet_id })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        bytes.append(&mut self.fixed_header());
        bytes.append(&mut self.variable_header());
        bytes
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
    fn fixed_header(&self) -> Vec<u8> {
        let fixed_header_buffer: Vec<u8> = vec![CONTROL_BYTE, FIXED_REMAINING_LENGTH];
        fixed_header_buffer
    }

    #[doc(hidden)]
    /// The variable header contains the Packet Identifier of the UNSUBSCRIBE Packet that is being acknowledged
    fn variable_header(&self) -> Vec<u8> {
        let mut variable_header_buffer: Vec<u8> = vec![];
        let packet_id_representation = self.packet_id.to_be_bytes();
        variable_header_buffer.push(packet_id_representation[0]);
        variable_header_buffer.push(packet_id_representation[1]);
        variable_header_buffer
    }
}

#[cfg(test)]
mod tests {}
