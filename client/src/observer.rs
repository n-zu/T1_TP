use packets::{puback::Puback, publish::Publish, suback::Suback};

use crate::{
    client::ClientError,
    client_packets::{Connack, Unsuback},
};

#[derive(Debug)]
pub enum Message {
    Connected(Result<Connack, ClientError>),
    Subscribed(Result<Suback, ClientError>),
    Unsubscribed(Result<Unsuback, ClientError>),
    Published(Result<Option<Puback>, ClientError>),
    Publish(Publish),
    InternalError(ClientError),
}

pub trait Observer: Clone + Send + Sync + 'static {
    fn update(&self, msg: Message);
}
