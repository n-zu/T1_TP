#[doc(hidden)]
const PINGREQ_PACKET_TYPE: u8 = 0b11000000;
#[doc(hidden)]
const REMAINING_LENGTH: u8 = 0b00000000;
#[doc(hidden)]
const RESERVED_BYTES_MASK: u8 = 0b00001111;

pub struct PingReq;

impl PingReq {
    pub fn new() -> PingReq {
        PingReq
    }

    pub fn encode(&self) -> Vec<u8> {
        vec![PINGREQ_PACKET_TYPE, REMAINING_LENGTH]
    }
}

#[cfg(test)]
mod tests {
    use super::{PingReq, RESERVED_BYTES_MASK};

    #[test]
    fn test_packet_type() {
        let bytes = PingReq::new().encode();
        let packet_type = bytes[0] >> 4;
        assert_eq!(packet_type, 12);
    }

    #[test]
    fn test_reserved_bytes_are_zero() {
        let bytes = PingReq::new().encode();
        let reserved_bytes = bytes[0] & RESERVED_BYTES_MASK;
        assert_eq!(reserved_bytes, 0);
    }

    #[test]
    fn test_remaining_length_is_zero() {
        let bytes = PingReq::new().encode();
        let remaining_length = bytes[1];
        assert_eq!(remaining_length, 0);
    }
}
