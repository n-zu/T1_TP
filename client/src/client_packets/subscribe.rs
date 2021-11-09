#![allow(dead_code)]

use packets::{
    packet_reader::{PacketError, QoSLevel, RemainingLength},
    utf8::Field,
};

const SUBSCRIBE_PACKET_TYPE: u8 = 0x80;
const FIXED_FLAGS: u8 = 2;

#[derive(Debug)]
pub struct Topic {
    /// Topic for a subsribe packet
    name: Field,
    qos: QoSLevel,
}

impl Topic {
    /// Creates a new topic
    /// Returns PacketError if the topic name is invalid
    pub fn new(name: &str, qos: QoSLevel) -> Result<Topic, PacketError> {
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
}

#[derive(Debug)]
pub struct Subscribe {
    /// Client-side subscribe packet structure
    packet_identifier: u16,
    topics: Vec<Topic>,
}

impl Subscribe {
    /// Creates a new subscribe packet
    pub fn new(topics: Vec<Topic>, packet_identifier: u16) -> Subscribe {
        Subscribe {
            topics,
            packet_identifier,
        }
    }

    /// Returns the subscribe packet remaining bytes length
    fn remaining_length(&self) -> usize {
        let mut len = 2; // Packet Identifier
        for topic in self.topics.iter() {
            len += 2; // Topic Length Bytes
            len += topic.len(); // Topic Name + QoS
        }
        len
    }

    /// Returns the subscribe packet encoded bytes
    pub fn encode(&self) -> Result<Vec<u8>, PacketError> {
        let mut packet = vec![
            // Packet Type and Flags
            SUBSCRIBE_PACKET_TYPE | FIXED_FLAGS,
        ];

        // Remaining Length
        packet.append(&mut RemainingLength::encode(
            &RemainingLength::from_uncoded(self.remaining_length())?,
        ));

        // Packet Identifier
        packet.push((self.packet_identifier >> 8) as u8);
        packet.push((self.packet_identifier & 0xFF) as u8);

        // Payload: Topic Filters
        for topic in self.topics.iter() {
            // Topic name & Length
            packet.append(&mut topic.encode());
        }
        Ok(packet)
    }

    // TODO: test
    pub fn max_qos(&self) -> QoSLevel {
        let mut max = QoSLevel::QoSLevel0;
        for topic in self.topics.iter() {
            if topic.qos as u8 > max as u8 {
                max = topic.qos;
            }
        }
        max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_encode_1_topic() {
        let topic = Topic::new("topic", QoSLevel::QoSLevel1).unwrap();
        let topics = vec![topic];
        let subscribe = Subscribe::new(topics, 1);
        let packet = subscribe.encode().unwrap();
        assert_eq!(
            packet,
            [
                0b10000010, // Packet Type and Flags
                10,         // Remaining Length 10 = +2 +2 +5 +1
                0, 1, // Packet Identifier
                0, 5, // Topic Length
                116, 111, 112, 105, 99, // Topic Name
                1,  // Topic QoS
            ]
        );
    }

    #[test]
    fn test_subscribe_encode_2_topics() {
        let topic1 = Topic::new("topic1", QoSLevel::QoSLevel0).unwrap();
        let topic2 = Topic::new("topic2", QoSLevel::QoSLevel1).unwrap();
        let topics = vec![topic1, topic2];
        let subscribe = Subscribe::new(topics, 2);
        let packet = subscribe.encode().unwrap();
        assert_eq!(
            packet,
            [
                0x82, // Packet Type and Flags
                20,   // Remaining Length 10 = +2 +9 +9
                0, 2, // Packet Identifier
                0, 6, // Topic 1 Length
                116, 111, 112, 105, 99, 49, // Topic 1 Name
                0,  // Topic 1 QoS
                0, 6, // Topic 2 Length
                116, 111, 112, 105, 99, 50, // Topic 2 Name
                1,  // Topic 2 QoS
            ]
        );
    }
}
