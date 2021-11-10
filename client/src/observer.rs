use packets::{puback::Puback, publish::Publish, suback::Suback};

use crate::{client::ClientError, client_packets::Connack};

pub enum Message {
    Connected(Result<Connack, ClientError>),
    Subscribed(Result<Suback, ClientError>),
    Published(Result<Option<Puback>, ClientError>),
    Publish(Publish),
    InternalError(ClientError),
}

pub trait Observer: Clone + Send + Sync {
    fn update(&self, msg: Message);
}
