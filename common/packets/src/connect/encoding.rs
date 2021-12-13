#![allow(dead_code)]

use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::{PacketError, PacketResult},
    packet_reader::RemainingLength,
    traits::{MQTTBytes, MQTTEncoding},
    utf8::Field,
};

use super::*;

#[doc(hidden)]
const CONTINUATION_SHIFT: u8 = 7;
#[doc(hidden)]
const ENCODED_LEN_MAX_BYTES: usize = 4;
#[doc(hidden)]
const PACKET_TYPE_SHIFT: u8 = 4;
#[doc(hidden)]
const USER_NAME_WITHOUT_PASSWORD_MSG: &str =
    "Se intento crear un paquete con user_name pero sin password";

/// Connect packet builder
pub struct ConnectBuilder {
    #[doc(hidden)]
    connect: Connect,
}

impl MQTTEncoding for Connect {
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let mut encoded = vec![];
        encoded.append(&mut self.fixed_header()?);
        encoded.append(&mut self.variable_header());
        encoded.append(&mut self.payload()?);
        Ok(encoded)
    }
}

impl Connect {
    #[doc(hidden)]
    fn protocol_name(&self) -> MQTTBytes {
        Field::new_from_string("MQTT")
            .expect("Error inesperado")
            .encode()
    }

    #[doc(hidden)]
    fn protocol_level(&self) -> u8 {
        PROTOCOL_LEVEL_3_1_1
    }

    #[doc(hidden)]
    fn flags(&self) -> u8 {
        let mut flags = 0;
        if self.user_name.is_some() {
            flags |= USER_NAME_PRESENT;
        }
        if self.password.is_some() {
            flags |= PASSWORD_PRESENT;
        }
        if let Some(last_will) = &self.last_will {
            if last_will.retain_flag {
                flags |= WILL_RETAIN;
            }

            flags |= (last_will.qos as u8) << WILL_QOS_SHIFT;

            flags |= LAST_WILL_PRESENT;
        }
        if self.clean_session {
            flags |= CLEAN_SESSION;
        }

        flags
    }

    #[doc(hidden)]
    fn payload(&self) -> PacketResult<MQTTBytes> {
        let mut payload = vec![];
        payload.append(&mut Field::new_from_string(&self.client_id)?.encode());
        if let Some(last_will) = &self.last_will {
            payload.append(&mut Field::new_from_string(&last_will.topic_name)?.encode());
            payload.append(&mut Field::new_from_string(&last_will.topic_message)?.encode());
        }
        if let Some(user_name) = &self.user_name {
            payload.append(&mut Field::new_from_string(user_name)?.encode());
        }
        if let Some(password) = &self.password {
            payload.append(&mut Field::new_from_string(password)?.encode());
        }
        Ok(payload)
    }

    #[doc(hidden)]
    fn variable_header(&self) -> MQTTBytes {
        let mut variable_header = vec![];
        variable_header.append(&mut self.protocol_name());
        variable_header.push(self.protocol_level());
        variable_header.push(self.flags());
        variable_header.push(self.keep_alive.to_be_bytes()[0]);
        variable_header.push(self.keep_alive.to_be_bytes()[1]);
        variable_header
    }

    #[doc(hidden)]
    fn fixed_header(&self) -> PacketResult<MQTTBytes> {
        let mut fixed_header = vec![];
        let remaining_len = self.variable_header().len() + self.payload()?.len();
        let mut remaining_len = RemainingLength::from_uncoded(remaining_len)?.encode();
        let control_byte = build_control_byte(PacketType::Connect, RESERVED_BITS);
        fixed_header.push(control_byte);
        fixed_header.append(&mut remaining_len);
        Ok(fixed_header)
    }
}

impl ConnectBuilder {
    /// Creates a ConnectBuilder
    ///
    /// # Errors
    ///
    /// Returns error if the length of the client_id exceeds
    /// the maximum established for utf8 fields in MQTT V3.1.1
    /// protocol
    pub fn new(client_id: &str, keep_alive: u16, clean_session: bool) -> PacketResult<Self> {
        Ok(ConnectBuilder {
            connect: Connect {
                client_id: client_id.to_owned(),
                clean_session,
                user_name: None,
                password: None,
                last_will: None,
                keep_alive,
            },
        })
    }

    pub fn user_name(mut self, user_name: &str) -> PacketResult<Self> {
        self.connect.user_name = Some(user_name.to_owned());
        Ok(self)
    }

    pub fn password(mut self, password: &str) -> PacketResult<Self> {
        self.connect.password = Some(password.to_owned());
        Ok(self)
    }

    pub fn last_will(mut self, last_will: LastWill) -> Self {
        self.connect.last_will = Some(last_will);
        self
    }

    /// Builds the packet with the received parameters
    ///
    /// # Errors
    ///
    /// Returns error if the packet fields do not meet the
    /// requirements of the MQTT V3.1.1 protocol
    pub fn build(self) -> PacketResult<Connect> {
        if self.connect.password.is_some() && self.connect.user_name.is_none() {
            return Err(PacketError::new_msg(USER_NAME_WITHOUT_PASSWORD_MSG));
        }

        Ok(self.connect)
    }
}
