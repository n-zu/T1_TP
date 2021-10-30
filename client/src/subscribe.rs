#![allow(dead_code)]

use packets::{packet_reader::PacketError, utf8::Field};

const SUBSCRIBE_PACKET_TYPE: u8 = 0x80;
const FIXED_FLAGS: u8 = 2;

#[derive(Debug)]
pub struct Topic {
    // Topic for a subsribe packet
    name: Field,
    qos: u8,
}

impl Topic {
    // Creates a new topic
    // Returns PacketError if the topic name is invalid
    pub fn new(name: &str, qos: u8) -> Result<Topic, PacketError> {
        Ok(Topic {
            name: Field::new_from_string(name)?,
            qos,
        })
    }

    // Returns the topic name
    pub fn name(&self) -> &Field {
        &self.name
    }

    // Returns the topic qos
    pub fn qos(&self) -> u8 {
        self.qos
    }

    // Returns the topic length
    pub fn len(&self) -> usize {
        self.name.encode().len()
        - 2 // remove 2 bytes from ecoded length 
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
    // Creates a new subscribe packet
    pub fn new(topics: Vec<Topic>, packet_identifier: u16) -> Subscribe {
        Subscribe {
            topics,
            packet_identifier,
        }
    }

    // Unused Getters
    /*
    pub fn topics(&self) -> &Vec<Topic> {
        &self.topics
    }

    pub fn packet_identifier(&self) -> u16 {
        self.packet_identifier
    }
    */

    // Returns the subscribe packet remaining bytes length
    fn remaining_length(&self) -> usize {
        let mut len = 2; // Packet Identifier
        for topic in self.topics.iter() {
            len += 2; // Topic Length Bytes
            len += topic.len(); // Topic Name + QoS
        }
        len
    }

    // Returns the subscribe packet encoded bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut packet = Vec::new();

        // Packet Type and Flags
        packet.push(SUBSCRIBE_PACKET_TYPE | FIXED_FLAGS);

        // Remaining Length
        packet.push(self.remaining_length() as u8); // FIXME : Use RemainingLenght

        // Packet Identifier
        packet.push((self.packet_identifier >> 8) as u8);
        packet.push((self.packet_identifier & 0xFF) as u8);

        // Payload: Topic Filters
        for topic in self.topics.iter() {
            // Topic name & Length
            packet.append(&mut topic.name.encode());
            // Topic QoS
            packet.push(topic.qos);
        }
        packet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_encode_1_topic() {
        let topic = Topic::new("topic", 1).unwrap();
        let topics = vec![topic];
        let subscribe = Subscribe::new(topics, 1);
        let packet = subscribe.encode();
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
        let topic1 = Topic::new("topic1", 0).unwrap();
        let topic2 = Topic::new("topic2", 1).unwrap();
        let topics = vec![topic1, topic2];
        let subscribe = Subscribe::new(topics, 2);
        let packet = subscribe.encode();
        assert_eq!(
            packet,
            [
                0b10000010, // Packet Type and Flags
                20,         // Remaining Length 10 = +2 +9 +9
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
