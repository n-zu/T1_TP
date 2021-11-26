#![allow(dead_code)]

use super::*;
use crate::{
    helpers::{build_control_byte, PacketType},
    packet_error::PacketResult,
    packet_reader::RemainingLength,
    topic::Topic,
    traits::{MQTTBytes, MQTTEncoding},
};

const FIXED_FLAGS: u8 = 2;

impl MQTTEncoding for Subscribe {
    /// Returns the subscribe packet encoded bytes
    fn encode(&self) -> PacketResult<MQTTBytes> {
        let control_byte = build_control_byte(PacketType::Subscribe, RESERVED_BITS);
        let mut packet = vec![
            // Packet Type and Flags
            control_byte,
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
}
