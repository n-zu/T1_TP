use crate::packet_error::{ErrorKind, PacketError, PacketResult};

mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";
#[doc(hidden)]
const RESERVED_BITS: u8 = 0;

#[derive(Debug, PartialEq)]
/// The UNSUBACK Packet is sent by the Server to the Client
/// to confirm receipt of an UNSUBSCRIBE Packet.
pub struct Unsuback {
    packet_id: u16,
}

impl Unsuback {
    /// Returns the packet identifier
    pub fn packet_id(&self) -> u16 {
        self.packet_id
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
}
