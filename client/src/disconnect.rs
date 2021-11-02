#![allow(dead_code)]

#[doc(hidden)]
const DISCONNECT_PACKET_TYPE: u8 = 0b11100000;
#[doc(hidden)]
const DISCONNECT_RESERVED_BYTES: u8 = 0b00000000;
#[doc(hidden)]
const DISCONNECT_REMAINING_LENGTH: u8 = 0;

/// The Disconnect packet is the final packet sent from the Client to the Server.
/// It indicates that the Client is disconnecting cleanly.
pub struct Disconnect();

impl Disconnect {
    /// Creates a new Disconnect packet
    pub fn new() -> Disconnect {
        Disconnect()
    }

    /// Encodes the Disconnect packet according to the
    /// MQTT V3.1.1 protocol
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = vec![DISCONNECT_PACKET_TYPE | DISCONNECT_RESERVED_BYTES];
        encoded.push(DISCONNECT_REMAINING_LENGTH);
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::Disconnect;

    #[test]
    fn test_control_byte() {
        let packet = Disconnect::new();
        let control_byte = packet.encode()[0];
        let expected_control_byte = 0b11100000;
        assert_eq!(control_byte, expected_control_byte);
    }

    #[test]
    fn test_remaining_length_should_be_zero() {
        let packet = Disconnect::new();
        let remaining_length = packet.encode()[1];
        let expected_control_byte = 0;
        assert_eq!(remaining_length, expected_control_byte);
    }

    #[test]
    fn test_packet_should_be_two_bytes_long() {
        let packet = Disconnect::new();
        let encoded = packet.encode();
        assert_eq!(encoded.len(), 2);
    }
}
