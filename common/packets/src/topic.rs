use crate::packet_error::{ErrorKind, PacketError, PacketResult};
use crate::qos::QoSLevel;
use crate::utf8::Field;

#[derive(Debug, Clone)]
pub struct Topic {
    /// Topic for a subsribe packet
    name: Field,
    qos: QoSLevel,
}

impl Topic {
    /// Creates a new topic
    /// Returns PacketError if the topic name is invalid
    pub fn new(name: &str, qos: QoSLevel) -> PacketResult<Topic> {

        Topic::check_valid_topic_name(name)?;

        Ok(Topic {
            name: Field::new_from_string(name)?,
            qos,
        })
    }

    /// Encodes the topic and returns the encoded bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.name.encode());
        bytes.push(self.qos as u8);
        bytes
    }

    // Returns the topic length
    pub fn len(&self) -> usize {
        self.name.encode().len()
        - 2 // remove 2 bytes from encoded length 
        + 1 // add 1 byte for qos
    }
    // Returns if topic is empty
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }

    /// Returns the topic name
    pub fn name(&self) -> &str {
        self.name.decode()
    }

    /// Returns the topic qos
    pub fn qos(&self) -> QoSLevel {
        self.qos
    }

    pub fn set_max_qos(&mut self, max_qos: QoSLevel) {
        if (self.qos as u8) > (max_qos as u8) {
            self.qos = max_qos;
        }
    }

    fn check_valid_topic_name(name: &str) -> PacketResult<()> {
        if name.is_empty() {
            return Err(PacketError::new_kind(
                "Topic name is empty",
                ErrorKind::InvalidTopicName
            ));
        }

        if name.starts_with('$') {
            return Err(PacketError::new_kind(
                "Topic name cannot start with $",
                ErrorKind::InvalidTopicName
            ));
        }

        Ok(())
    }

}
