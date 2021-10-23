use std::io::Read;
use std::result::Result;

#[derive(Debug, Eq, PartialEq)]
pub enum ConnackError {
    WrongEncoding(String),
    UnacceptableProtocolVersion,
    IdentifierRejected,
    ServerUnavailable,
    BadUserNameOrPassword,
    NotAuthorized,
}

#[derive(Debug, PartialEq)]
pub struct Connack {
    session_present: u8,
    return_code: u8,
}

const CONNACK_FIXED_FIRST_BYTE: u8 = 32; // 0010 0000
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

        if session_present != 1 && session_present != 0 {
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

#[cfg(test)]
mod tests {
    use crate::connack::{Connack, ConnackError};
    use std::io::Cursor;

    #[test]
    fn test_should_raise_wrong_encoding_error_at_first_byte() {
        let v: Vec<u8> = vec![145, 3];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        assert_eq!(
            result,
            ConnackError::WrongEncoding(
                "First byte doesn't follow MQTT 3.1.1 protocol".to_string()
            )
        );
    }

    #[test]
    fn test_should_raise_wrong_encoding_error_at_remaining_length() {
        let v: Vec<u8> = vec![32, 3];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        assert_eq!(
            result,
            ConnackError::WrongEncoding(
                "Remaining length doesn't follow MQTT 3.1.1 protocol".to_string()
            )
        );
    }

    #[test]
    fn test_should_raise_wrong_encoding_error_at_unknown_connack_flag() {
        let v: Vec<u8> = vec![32, 2, 3];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        let error_expected = ConnackError::WrongEncoding(
            "Unknown session present flag for MQTT 3.1.1 protocol".to_string(),
        );
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_1_should_raise_unacceptable_protocol_version_error() {
        let v: Vec<u8> = vec![32, 2, 0, 1];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        let error_expected = ConnackError::UnacceptableProtocolVersion;
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_2_should_raise_identifier_rejected_error() {
        let v: Vec<u8> = vec![32, 2, 1, 2];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        let error_expected = ConnackError::IdentifierRejected;
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_3_should_raise_server_unavailable_error() {
        let v: Vec<u8> = vec![32, 2, 1, 3];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        let error_expected = ConnackError::ServerUnavailable;
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_4_should_raise_bad_username_or_password_error() {
        let v: Vec<u8> = vec![32, 2, 1, 4];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        let error_expected = ConnackError::BadUserNameOrPassword;
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_return_code_5_should_raise_not_authorized_error() {
        let v: Vec<u8> = vec![32, 2, 1, 5];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream).unwrap_err();
        let error_expected = ConnackError::NotAuthorized;
        assert_eq!(result, error_expected);
    }

    #[test]
    fn test_should_return_valid_connack_packet_with_session_present_0() -> Result<(), ConnackError>
    {
        let v: Vec<u8> = vec![32, 2, 0, 0];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream)?;
        let connack_expected = Connack {
            session_present: 0,
            return_code: 0,
        };
        assert_eq!(result, connack_expected);
        Ok(())
    }

    #[test]
    fn test_should_return_valid_connack_packet_with_session_present_1() -> Result<(), ConnackError>
    {
        let v: Vec<u8> = vec![32, 2, 1, 0];
        let mut stream = Cursor::new(v);
        let result = Connack::read_from(&mut stream)?;
        let connack_expected = Connack {
            session_present: 1,
            return_code: 0,
        };
        assert_eq!(result, connack_expected);
        Ok(())
    }
}
