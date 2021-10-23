use std::io::Read;
use std::result::Result;

#[derive(Debug)]
pub enum ConnackError {
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

const CONNACK_FIXED_FIRST_BYTE: u8 = 32;
const CONNACK_FIXED_REMAINING_LENGTH: u8 = 2;
const CONNACK_UNACCEPTABLE_PROTOCOL: u8 = 1;
const CONNACK_IDENTIFIER_REJECTED: u8 = 2;
const CONNACK_SERVER_UNAVAILABLE: u8 = 3;
const CONNACK_BAD_USER_NAME_OR_PASSWORD: u8 = 4;
const CONNACK_NOT_AUTHORIZED: u8 = 5;

impl Connack {
    /// Devuelve un Connack con estado valido leyendo bytes desde el stream
    ///
    /// # Errors
    ///
    /// Si la estructura de los bytes enviados por el stream no se corresponde con el estandar de
    /// MQTT 3.1.1, se devuelve un ConnackError::WrongEncoding(mensaje)
    ///
    /// Si el return_code no es 0, entonces se devuelve un ConnackError especifico correspondiente
    /// al estandar de MQTT
    pub fn read_from(stream: &mut impl Read) -> Result<Connack, ConnackError> {
        let mut buffer = [0u8; 1];
        // first byte packet control type
        stream.read_exact(&mut buffer);
        let first_byte_control_packet_type = u8::from_be_bytes(buffer);
        if first_byte_control_packet_type != CONNACK_FIXED_FIRST_BYTE {
            return Err(ConnackError::WrongEncoding(
                "First byte doesn't follow MQTT 3.1.1 protocol".to_string(),
            ));
        }
        // remaining length
        stream.read_exact(&mut buffer);
        let remaining_length = u8::from_be_bytes(buffer);
        if remaining_length != CONNACK_FIXED_REMAINING_LENGTH {
            return Err(ConnackError::WrongEncoding(
                "Remaining length doesn't follow MQTT 3.1.1 protocol".to_string(),
            ));
        }
        // connack flag
        stream.read_exact(&mut buffer);
        let session_present = u8::from_be_bytes(buffer);

        if session_present != 1 || session_present != 0 {
            return Err(ConnackError::WrongEncoding(
                "Unknown session present flag for MQTT 3.1.1 protocol".to_string(),
            ));
        }

        //connect return code
        stream.read_exact(&mut buffer);
        let return_code = u8::from_be_bytes(buffer);
        match return_code {
            CONNACK_UNACCEPTABLE_PROTOCOL => Err(ConnackError::UnacceptableProtocolVersion),
            CONNACK_IDENTIFIER_REJECTED => Err(ConnackError::IdentifierRejected),
            CONNACK_SERVER_UNAVAILABLE => Err(ConnackError::ServerUnavailable),
            CONNACK_BAD_USER_NAME_OR_PASSWORD => Err(ConnackError::BadUserNameOrPassword),
            CONNACK_NOT_AUTHORIZED => Err(ConnackError::NotAuthorized),
            _ => Ok(Self {
                return_code,
                session_present,
            }),
        }
    }
}
