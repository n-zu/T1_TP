mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

#[doc(hidden)]
const MSG_INVALID_PACKET_ID: &str = "Packet identifier must be greater than zero";
#[doc(hidden)]
const RESERVED_BITS: u8 = 0b10;
#[doc(hidden)]
const MSG_AT_LEAST_ONE_TOPIC_FILTER: &str =
    "Unsubscribe packet must contain at least one topic filter";

#[doc(hidden)]
const MSG_AT_LEAST_ONE_CHAR_LONG_TOPIC_FILTER: &str =
    "Topic filter must be at least one character long";

#[derive(Debug)]
/// Unsubscribe packet struct
pub struct Unsubscribe {
    packet_id: u16,
    topic_filters: Vec<String>,
}

impl Unsubscribe {
    /// Gets packet id from current Unsubscribe packet
    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }

    /// Gets topic filters from current Unsubscribe packet
    pub fn topic_filters(&self) -> &Vec<String> {
        &self.topic_filters
    }
}
