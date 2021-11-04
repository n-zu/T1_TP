#![allow(dead_code)]

/// A PingResp Packet is sent by the Server to the Client in response
/// to a PingReq Packet.
/// It indicates that the Server is alive.
pub struct PingResp;

#[doc(hidden)]
const PINGRESP_PACKET_TYPE: u8 = 0b11010000;
#[doc(hidden)]
const PINGRESP_RESERVED_BYTES: u8 = 0b00000000;
#[doc(hidden)]
const REMAINING_LENGTH: u8 = 0b00000000;

impl PingResp {
    /// Creates a new PingResp packet
    pub fn new() -> PingResp {
        PingResp
    }

    /// Encodes a PingResp packet
    pub fn encode(&self) -> Vec<u8> {
        vec![
            PINGRESP_PACKET_TYPE | PINGRESP_RESERVED_BYTES,
            REMAINING_LENGTH,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::PingResp;

    #[test]
    fn test_control_byte() {
        let encoded = PingResp::new().encode();
        let control_byte = encoded[0];
        assert_eq!(control_byte, 0b11010000);
    }

    #[test]
    fn test_remaining_length() {
        let encoded = PingResp::new().encode();
        let remaining_length = encoded[1];
        assert_eq!(remaining_length, 0b00000000);
    }

    #[test]
    fn test_packet_should_be_two_bytes_long() {
        let encoded = PingResp::new().encode();
        assert_eq!(encoded.len(), 2);
    }
}
