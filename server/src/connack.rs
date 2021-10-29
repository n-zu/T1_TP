#[derive(Debug, Clone, Copy)]
pub struct Connack {
    session_present: u8,
    return_code: u8,
}

const CONNACK_FIXED_FIRST_BYTE: u8 = 32; // 0010 0000
const CONNACK_FIXED_REMAINING_LENGTH: u8 = 2;

pub const CONNACK_CONNECTION_ACCEPTED: u8 = 0;
// pub const CONNACK_UNACCEPTABLE_PROTOCOL: u8 = 1;
// pub const CONNACK_IDENTIFIER_REJECTED: u8 = 2;
// pub const CONNACK_SERVER_UNAVAILABLE: u8 = 3;
// pub const CONNACK_BAD_USER_NAME_OR_PASSWORD: u8 = 4;
// pub const CONNACK_NOT_AUTHORIZED: u8 = 5;

impl Connack {
    pub fn new(session_present: u8, return_code: u8) -> Connack {
        Connack {
            session_present,
            return_code,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        vec![
            CONNACK_FIXED_FIRST_BYTE,
            CONNACK_FIXED_REMAINING_LENGTH,
            self.session_present,
            self.return_code,
        ]
    }
}
