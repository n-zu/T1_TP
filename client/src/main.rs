use std::io::Read;
use std::result::Result;

#[derive(Debug)]
enum ConnackError {
    WrongEncoding(String),
    UnacceptableProtocolVersion,
    IdentifierRejected,
    ServerUnavailable,
    BadUserNameOrPassword,
    NotAuthorized,
}

#[derive(Debug)]
pub struct Connack {
    session_present: u8,
    return_code: u8,
}

static CONNACK_FIXED_FIRST_BYTE: u8 = 32;
static CONNACK_FIXED_REMAINING_LENGTH: u8 = 2;

impl Connack {
    fn new() {}

    fn read_from(readable: &mut impl Read) -> Result<Connack, ConnackError> {
        let mut buffer = [0u8; 1];
        // first byte packet control type
        readable.read_exact(&mut buffer);
        let first_byte_control_packet_type = u8::from_be_bytes(buffer);
        if first_byte_control_packet_type != CONNACK_FIXED_FIRST_BYTE {
            return Err(ConnackError::WrongEncoding(
                "First byte doesn't follow MQTT 3.1.1 protocol".to_string(),
            ));
        }
        // remaining length
        readable.read_exact(&mut buffer);
        let remaining_length = u8::from_be_bytes(buffer);
        if remaining_length != CONNACK_FIXED_REMAINING_LENGTH {
            return Err(ConnackError::WrongEncoding(
                "Remaining length doesn't follow MQTT 3.1.1 protocol".to_string(),
            ));
        }
        // connack flag
        readable.read_exact(&mut buffer);
        let session_present = u8::from_be_bytes(buffer);

        if session_present != 1 || session_present != 0 {
            return Err(ConnackError::WrongEncoding(
                "Unknown session present flag for this MQTT 3.1.1 protocol".to_string(),
            ));
        }

        //connect return code
        readable.read_exact(&mut buffer);
        let return_code = u8::from_be_bytes(buffer);
        match return_code {
            0 => Ok(Self {
                session_present,
                return_code,
            }),
            1 => {
                return Err(ConnackError::UnacceptableProtocolVersion);
            }
            2 => {
                return Err(ConnackError::IdentifierRejected);
            }
            3 => {
                return Err(ConnackError::ServerUnavailable);
            }
            4 => {
                return Err(ConnackError::BadUserNameOrPassword);
            }
            5 => {
                return Err(ConnackError::NotAuthorized);
            }
            _ => {()};
        }
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sample_client() {
        assert_eq!(1, 1)
    }
}
