use crate::{
    packet_error::{ErrorKind, PacketError, PacketResult},
    topic_filter::TopicFilter,
};

mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const RESERVED_BITS: u8 = 0;
#[doc(hidden)]
const MSG_INVALID_RETURN_CODE: &str = "Allowed return codes are 0x00, 0x01, 0x80";
#[doc(hidden)]
const SUCCESS_MAXIMUM_QOS_0: u8 = 0;
#[doc(hidden)]
const SUCCESS_MAXIMUM_QOS_1: u8 = 1;
#[doc(hidden)]
const FAILURE: u8 = 0x80;

#[derive(Debug)]
/// Client/Server side structure for Suback packet
pub struct Suback {
    return_codes: Vec<u8>,
    subscribe_packet_id: u16,
    topics: Vec<TopicFilter>,
}

impl Suback {
    /// Get the suback's subscribe packet id.
    pub fn packet_id(&self) -> u16 {
        self.subscribe_packet_id
    }

    /// Set the suback's subscribe topics
    pub fn set_topics(&mut self, topics: Vec<TopicFilter>) {
        self.topics = topics;
    }
    /// Get the suback's subscribe topics
    pub fn topics(&self) -> &Vec<TopicFilter> {
        &self.topics
    }

    #[doc(hidden)]
    fn verify_return_codes_from_vec(return_codes: &[u8]) -> PacketResult<()> {
        for code in return_codes {
            Self::verify_return_code(code)?;
        }
        Ok(())
    }

    #[doc(hidden)]
    fn verify_return_code(return_code: &u8) -> PacketResult<()> {
        if !Self::is_return_code_valid(return_code) {
            return Err(PacketError::new_kind(
                MSG_INVALID_RETURN_CODE,
                ErrorKind::InvalidReturnCode,
            ));
        }
        Ok(())
    }

    #[doc(hidden)]
    fn is_return_code_valid(return_code: &u8) -> bool {
        *return_code == SUCCESS_MAXIMUM_QOS_0
            || *return_code == SUCCESS_MAXIMUM_QOS_1
            || *return_code == FAILURE
    }
}
