use std::convert::TryFrom;

use crate::packet_error::PacketError;

mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const CONNACK_CONNECTION_ACCEPTED: u8 = 0;
#[doc(hidden)]
const CONNACK_UNACCEPTABLE_PROTOCOL: u8 = 1;
#[doc(hidden)]
const CONNACK_IDENTIFIER_REJECTED: u8 = 2;
#[doc(hidden)]
const CONNACK_SERVER_UNAVAILABLE: u8 = 3;
#[doc(hidden)]
const CONNACK_BAD_USER_NAME_OR_PASSWORD: u8 = 4;
#[doc(hidden)]
const CONNACK_NOT_AUTHORIZED: u8 = 5;
#[doc(hidden)]
const RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const MSG_INVALID_REMAINING_LENGTH: &str =
    "Remaining length does not follow MQTT v3.1.1 protocol for a Connack packet";
#[doc(hidden)]
const MSG_INVALID_SESSION_PRESENT_FLAG: &str =
    "Session flag does not follow MQTT v3.1.1 protocol for a Connack packet";
#[doc(hidden)]
const MSG_UNACCEPTABLE_PROTOCOL_VERSION: &str =
    "The server does not support the level of the MQTT protocol requested by the Client";
#[doc(hidden)]
const MSG_IDENTIFIER_REJECTED: &str =
    "The Client identifier is correct UTF-8 but not allowed by the Server";
#[doc(hidden)]
const MSG_SERVER_UNAVAILABLE: &str =
    "The Network connection has been made but the MQTT service is unavailable";
const MSG_BAD_USER_NAME_OR_PASSWORD: &str = "The data in the user name or password is malformed";
#[doc(hidden)]
const MSG_NOT_AUTHORIZED: &str = "The Client is not authorized to connect";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Connack {
    pub session_present: bool,
    pub return_code: ConnackReturnCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnackReturnCode {
    Accepted,
    UnacceptableProtocolVersion,
    IdentifierRejected,
    ServerUnavailable,
    BadUserNameOrPassword,
    NotAuthorized,
}

impl From<ConnackReturnCode> for u8 {
    fn from(code: ConnackReturnCode) -> Self {
        match code {
            ConnackReturnCode::Accepted => CONNACK_CONNECTION_ACCEPTED,
            ConnackReturnCode::UnacceptableProtocolVersion => CONNACK_UNACCEPTABLE_PROTOCOL,
            ConnackReturnCode::IdentifierRejected => CONNACK_IDENTIFIER_REJECTED,
            ConnackReturnCode::ServerUnavailable => CONNACK_SERVER_UNAVAILABLE,
            ConnackReturnCode::BadUserNameOrPassword => CONNACK_BAD_USER_NAME_OR_PASSWORD,
            ConnackReturnCode::NotAuthorized => CONNACK_NOT_AUTHORIZED,
        }
    }
}

impl TryFrom<u8> for ConnackReturnCode {
    type Error = PacketError;

    fn try_from(return_code_byte: u8) -> Result<Self, Self::Error> {
        match return_code_byte {
            CONNACK_CONNECTION_ACCEPTED => Ok(ConnackReturnCode::Accepted),
            CONNACK_UNACCEPTABLE_PROTOCOL => Ok(ConnackReturnCode::UnacceptableProtocolVersion),
            CONNACK_IDENTIFIER_REJECTED => Ok(ConnackReturnCode::IdentifierRejected),
            CONNACK_SERVER_UNAVAILABLE => Ok(ConnackReturnCode::ServerUnavailable),
            CONNACK_BAD_USER_NAME_OR_PASSWORD => Ok(ConnackReturnCode::BadUserNameOrPassword),
            CONNACK_NOT_AUTHORIZED => Ok(ConnackReturnCode::NotAuthorized),
            invalid => Err(PacketError::new_msg(&format!(
                "Codigo de retorno invalido ({})",
                invalid
            ))),
        }
    }
}
