use std::convert::TryFrom;

use crate::{packet_error::PacketResult, qos::QoSLevel, suback::Suback, topic::Topic};

mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const RESERVED_BITS: u8 = 2;

#[derive(Debug)]
pub struct Subscribe {
    packet_identifier: u16,
    topics: Vec<Topic>,
}

impl Subscribe {
    /// Get a the subscribe's packet identifier.
    pub fn packet_identifier(&self) -> u16 {
        self.packet_identifier
    }

    /// Get the subscribe's topics.
    pub fn topics(&self) -> Vec<Topic> {
        self.topics.clone()
    }

    /// Gets the QoSLevel of a topic filter
    fn get_qos(byte: u8) -> PacketResult<QoSLevel> {
        QoSLevel::try_from(byte)
    }

    /// Creates a response packet (Suback in this case) for this Subscribe packet
    ///
    /// # Errors
    ///
    /// Allowed return codes are 0x00, 0x01, 0x80. If a return code doesn't match any of those, this function returns a [ErrorKind::InvalidReturnCode]
    pub fn response(&self) -> PacketResult<Suback> {
        let mut return_codes = Vec::new();
        for topic in &self.topics {
            return_codes.push(topic.qos() as u8);
        }
        Suback::new_from_vec(return_codes, self.packet_identifier)
    }

    #[doc(hidden)]
    /// Sets max QoS for each Topic Filter in a Subscribe packet
    /// This is intended to be used by the server in case some QoS is not yet implemented by it
    pub fn set_max_qos(&mut self, max_qos: QoSLevel) {
        for topic in self.topics.iter_mut() {
            topic.set_max_qos(max_qos);
        }
    }
}
