mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const MSG_PACKET_MORE_BYTES_THAN_EXPECTED: &str = "Puback packet contains more bytes than expected";
#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";

#[derive(Debug, PartialEq)]
pub struct Puback {
    packet_id: u16,
}

impl Puback {
    /// Returns the Packet Id
    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }
}
