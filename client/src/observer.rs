use packets::{puback::Puback, publish::Publish, suback::Suback};

use crate::{
    client_error::ClientError,
    client_packets::{Connack, ConnackError},
};

pub enum Message {
    Connected(Connack),
    ConnectionRefused(ConnackError),
    Subscribed(Suback),
    Published(Puback),
    Publish(Publish),
    InternalError(ClientError),
}

pub trait Observer: Clone + Send {
    fn update(&self, msg: Message);
}
