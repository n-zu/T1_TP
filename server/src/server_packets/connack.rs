#[derive(Debug, Clone, Copy)]
pub enum ConnackReturnCode {
    Accepted,
    UnacceptableProtocolVersion,
    IdentifierRejected,
    ServerUnavailable,
    BadUserNameOrPassword,
    NotAuthorized,
}

#[derive(Debug, Clone, Copy)]
pub struct Connack {
    session_present: bool,
    return_code: ConnackReturnCode,
}

#[doc(hidden)]
const CONNACK_FIXED_FIRST_BYTE: u8 = 32; // 0010 0000
#[doc(hidden)]
const CONNACK_FIXED_REMAINING_LENGTH: u8 = 2;
#[doc(hidden)]
const CONNACK_SESSION_PRESENT_TRUE: u8 = 1;
#[doc(hidden)]
const CONNACK_SESSION_PRESENT_FALSE: u8 = 0;

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

impl Connack {
    pub fn new(session_present: bool, return_code: ConnackReturnCode) -> Connack {
        Connack {
            session_present,
            return_code,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![CONNACK_FIXED_FIRST_BYTE, CONNACK_FIXED_REMAINING_LENGTH];

        if self.session_present {
            bytes.push(CONNACK_SESSION_PRESENT_TRUE)
        } else {
            bytes.push(CONNACK_SESSION_PRESENT_FALSE)
        }
        bytes.push(u8::from(self.return_code));
        bytes
    }
}
