use crate::qos::QoSLevel;

mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

const MSG_TOPIC_NAME_ONE_CHAR: &str =
    "Topic name must be at least one character long for a Publish packet";
const MSG_TOPIC_WILDCARDS: &str = "Topic name must not have wildcards";
const MSG_DUP_FLAG_1_WITH_QOS_LEVEL_0: &str = "It can not be dup flag 1 with QoS level 0";

#[doc(hidden)]
const SINGLE_LEVEL_WILDCARD: char = '+';
#[doc(hidden)]
const MULTI_LEVEL_WILDCARD: char = '#';
#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";

#[derive(Debug, PartialEq, Clone)]
/// Publish packet structure for server/client side
pub struct Publish {
    pub packet_id: Option<u16>,
    pub topic_name: String,
    pub qos: QoSLevel,
    pub retain_flag: bool,
    pub dup_flag: bool,
    pub payload: String,
}

impl Publish {
    /// Gets packet_id from a Publish packet
    pub fn packet_id(&self) -> Option<u16> {
        self.packet_id
    }
    /// Gets topic_name from a Publish packet
    pub fn topic_name(&self) -> &str {
        &self.topic_name
    }
    /// Gets QoS from a Publish packet
    pub fn qos(&self) -> QoSLevel {
        self.qos
    }
    /// Gets retain_flag from a Publish packet
    pub fn retain_flag(&self) -> bool {
        self.retain_flag
    }
    /// Gets dup_flag from a Publish packet
    pub fn dup_flag(&self) -> bool {
        self.dup_flag
    }
    /// Gets the payload from a Publish packet
    pub fn payload(&self) -> &str {
        &self.payload
    }

    #[doc(hidden)]
    pub fn set_max_qos(&mut self, max_qos: QoSLevel) {
        if (max_qos as u8) < (self.qos as u8) {
            self.qos = max_qos;
        }
    }

    /// Set the publish's dup flag.
    pub fn set_dup(&mut self, dup: bool) {
        self.dup_flag = dup;
    }

    /// Set the publish's retain flag.
    pub fn set_retain_flag(&mut self, retain: bool) {
        self.retain_flag = retain;
    }
}
