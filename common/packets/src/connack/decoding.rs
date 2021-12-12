#![allow(dead_code)]

use std::io::Read;

use super::*;
use crate::{
    helpers::{check_packet_type, check_reserved_bits, PacketType},
    packet_error::{ErrorKind, PacketError, PacketResult},
    traits::MQTTDecoding,
};

#[doc(hidden)]
const CONNACK_CONTROL_TYPE: u8 = 0b10;
#[doc(hidden)]
const CONNACK_RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const CONNACK_FIXED_REMAINING_LENGTH: u8 = 0b10;
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

impl MQTTDecoding for Connack {
    /// Return a Connack with valid state from a stream of bytes
    ///
    /// # Arguments
    ///
    /// * `stream`: &mut impl Read
    ///
    /// returns: PacketResult<Connack>
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Cursor;
    /// use packets::connack::{Connack, ConnackReturnCode};
    /// use packets::traits::MQTTDecoding;
    ///
    /// let control_byte: u8 = 0b00100000;
    /// let v: Vec<u8> = vec![0b10, 0b01, 0b0];
    /// let mut stream = Cursor::new(v);
    /// let result = Connack::read_from(&mut stream, control_byte).unwrap();
    /// let connack_expected = Connack { session_present: true, return_code: ConnackReturnCode::Accepted };
    /// assert_eq!(result, connack_expected);
    /// ```
    /// # Errors
    ///
    /// This function returns a Packet error if:
    /// - the stream's bytes doesn't follow MQTT v3.1.1 protocol
    /// - return_code is not 0
    ///
    /// If return_code is not 0, this function returns a specific PacketError
    fn read_from<T: Read>(stream: &mut T, control_byte: u8) -> PacketResult<Connack> {
        let buffer = [0u8; 1];
        check_packet_type(control_byte, PacketType::Connack)?;
        check_reserved_bits(control_byte, CONNACK_RESERVED_BITS)?;
        Connack::verify_remaining_length(buffer, stream)?;
        let session_present = Connack::verify_session_present_flag(buffer, stream)?;
        let return_code = Connack::verify_return_code(buffer, stream)?;
        Ok(Self {
            return_code,
            session_present,
        })
    }
}

impl Connack {
    #[doc(hidden)]
    fn verify_remaining_length(mut buffer: [u8; 1], stream: &mut impl Read) -> PacketResult<()> {
        stream.read_exact(&mut buffer)?;
        let remaining_length = buffer[0];
        if remaining_length != CONNACK_FIXED_REMAINING_LENGTH {
            return Err(PacketError::new_kind(
                MSG_INVALID_REMAINING_LENGTH,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_session_present_flag(
        mut buffer: [u8; 1],
        stream: &mut impl Read,
    ) -> PacketResult<bool> {
        stream.read_exact(&mut buffer)?;
        let session_present = buffer[0];
        if session_present != 1 && session_present != 0 {
            return Err(PacketError::new_kind(
                MSG_INVALID_SESSION_PRESENT_FLAG,
                ErrorKind::InvalidProtocol,
            ));
        }
        Ok(session_present == 1)
    }

    #[doc(hidden)]
    fn verify_return_code(
        mut buffer: [u8; 1],
        stream: &mut impl Read,
    ) -> PacketResult<ConnackReturnCode> {
        stream.read_exact(&mut buffer)?;
        let return_code = buffer[0];
        match ConnackReturnCode::try_from(return_code)? {
            ConnackReturnCode::UnacceptableProtocolVersion => Err(PacketError::new_kind(
                MSG_UNACCEPTABLE_PROTOCOL_VERSION,
                ErrorKind::UnacceptableProtocolVersion,
            )),
            ConnackReturnCode::IdentifierRejected => Err(PacketError::new_kind(
                MSG_IDENTIFIER_REJECTED,
                ErrorKind::IdentifierRejected,
            )),
            ConnackReturnCode::ServerUnavailable => Err(PacketError::new_kind(
                MSG_SERVER_UNAVAILABLE,
                ErrorKind::ServerUnavailable,
            )),
            ConnackReturnCode::BadUserNameOrPassword => Err(PacketError::new_kind(
                MSG_BAD_USER_NAME_OR_PASSWORD,
                ErrorKind::BadUserNameOrPassword,
            )),
            ConnackReturnCode::NotAuthorized => Err(PacketError::new_kind(
                MSG_NOT_AUTHORIZED,
                ErrorKind::NotAuthorized,
            )),
            ConnackReturnCode::Accepted => Ok(ConnackReturnCode::Accepted),
        }
    }
}
