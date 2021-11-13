use packets::{puback::Puback, publish::Publish, suback::Suback};

use crate::{
    client::ClientError,
    client_packets::{Connack, Unsuback},
};

/// Messages for the Observer trait. They are intended
/// to inform the result of the send operations of the
/// client, except for the Publish message which should
/// be sent when the client receives a PUBLISH packet
/// and the InternalError which is a generic message
/// for general internal errors
#[derive(Debug)]
pub enum Message {
    Connected(Result<Connack, ClientError>),
    Subscribed(Result<Suback, ClientError>),
    Unsubscribed(Result<Unsuback, ClientError>),
    Published(Result<Option<Puback>, ClientError>),
    Publish(Publish),
    InternalError(ClientError),
}

/// Observer trait for the internal client
/// It may send messages of the relevant events
/// to its observer
pub trait Observer: Clone + Send + Sync + 'static {
    fn update(&self, msg: Message);
}
