use crate::packet_error::{ErrorKind, PacketError};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum QoSLevel {
    QoSLevel0,
    QoSLevel1,
    QoSLevel2,
}

impl From<QoSLevel> for u8 {
    fn from(qos: QoSLevel) -> u8 {
        match qos {
            QoSLevel::QoSLevel0 => 0,
            QoSLevel::QoSLevel1 => 1,
            QoSLevel::QoSLevel2 => 2,
        }
    }
}

impl TryFrom<u8> for QoSLevel {
    type Error = PacketError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(QoSLevel::QoSLevel0),
            1 => Ok(QoSLevel::QoSLevel1),
            2 => Ok(QoSLevel::QoSLevel2),
            _ => Err(PacketError::new_kind(
                "Invalid QoS level",
                ErrorKind::InvalidQoSLevel,
            )),
        }
    }
}
