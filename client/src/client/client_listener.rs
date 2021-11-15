use std::{
    io::{self, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use packets::{
    packet_reader::{ErrorKind, PacketError},
    puback::Puback,
    publish::Publish,
    suback::Suback,
};

use crate::{
    client::PendingAck,
    client_packets::{Connack, PingResp, Unsuback},
    observer::Observer,
};

use crate::observer::Message;

use super::{ClientError, STOP_TIMEOUT};

/// ReadTimeout trait from which the listener reads the packets
pub(crate) trait ReadTimeout: Read + Send + Sync + 'static {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
}

/// The packet listener of the client. It is responsible
/// for receiving all packets from the server, and
/// acknowledging the ones in which it is required.
pub(crate) struct ClientListener<T: Observer, R: ReadTimeout, A: AckSender> {
    stream: R,
    pending_ack: Arc<Mutex<Option<PendingAck>>>,
    observer: T,
    stop: Arc<AtomicBool>,
    ack_sender: Arc<A>,
}

enum PacketType {
    Connect,
    Connack,
    Publish,
    Puback,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    Pingreq,
    Pingresp,
    Disconnect,
}

/// Under which errors should the listener send
/// a Connected(Err()) to the observer instead of
/// stopping and sending an InternalError(Err())
const CONNECT_USER_ERRORS: [ErrorKind; 3] = [
    ErrorKind::BadUserNameOrPassword,
    ErrorKind::NotAuthorized,
    ErrorKind::IdentifierRejected,
];

// Acknowledge sender for the listener. Every time a packet
// which requires an acknowledgement is received, the listener
// will it through this sender.
pub(crate) trait AckSender {
    fn send_puback(&self, packet: Puback);
}

impl<T: Observer, R: ReadTimeout, A: AckSender> ClientListener<T, R, A> {
    /// Creates a new listener from the given stream, observer
    /// and pending_ack lock. It also receives a stop boolean
    /// to stop the listener when it is required.
    pub fn new(
        stream: R,
        pending_ack: Arc<Mutex<Option<PendingAck>>>,
        observer: T,
        stop: Arc<AtomicBool>,
        ack_sender: Arc<A>,
    ) -> Result<Self, ClientError> {
        stream.set_read_timeout(Some(STOP_TIMEOUT))?;

        Ok(Self {
            stream,
            pending_ack,
            observer,
            stop,
            ack_sender,
        })
    }

    /// Starts the listener. It reads the packets from the stream
    /// and writes the acknowledgements. In case of an internal error,
    /// it will send a Message::InternalError() to the observer and
    /// stop the listener, setting the stop boolean to true. Any packet
    /// error counts as an internal error (except those in CONNECT_USER_ERRORS,
    /// details below).  Every time it receives a packet, it reads all
    /// its contents from the stream. The way it handles the different
    /// packets is as follows:
    ///
    /// Suback, Unsuback, Puback: If the pending_ack enum lock contains
    /// Subscribe(), Unsubscribe() or Publish() correspondingly with a
    /// packet of the same identifier as the one it was received, it then
    /// sets pending_ack to None and sends a Message Subscribed(Ok()),
    /// Unsubscribed(Ok()) or Published(Ok(Some())) appropiately with the
    /// packet to the observer. In the case of the Puback, if a QoSLevel0
    /// Publish packet without an id was saved in the pending_ack lock,
    /// the listener will stop and send an InternalError() message to the
    /// observer with the error. In any other case, the packet is ignored.
    ///
    /// PingResp:  If the pending_ack enum lock contains PingReq(),
    /// it then sets the lock to None. Else the packet is ignored.
    ///
    /// Connack: If the connack packet wasn't read successfully and
    /// the error is not in CONNECT_USER_ERRORS, then an InternalError()
    /// is sent to the observer and the listener stops, as with any other
    /// packet.
    /// If the pending_ack enum lock contains Connect(), and the
    /// connack was read successfully, it sends a Connected(Ok()) to the
    /// observer. If it contains a Connect() and the connack wasn't read
    /// successfully, and the error kind is in CONNECT_USER_ERRORS, it sends
    /// a Connected(Err()) message to the observer with the error. In this
    /// two cases it sets the pending_ack lock to None. If the pending_ack
    /// didn't contain a Connect() in the first place, it ignores the packet.
    ///
    /// Publish: If the publish packet does not have an id (QoSLevel0), a
    /// Publish() message is sent to the observer with the packet. If it does
    /// have an id, it first tries to send the corresponding Puback packet to
    /// the server. If this fails, an InternalError() message is sent instead
    /// and the listener will stop.
    ///
    /// Any other packet will cause the listener to send an InternalError() to
    /// the observer and stop listening.
    pub fn wait_for_packets(&mut self) {
        while !self.stop.load(Ordering::Relaxed) {
            if let Err(err) = self.try_read_packet() {
                self.stop.store(true, Ordering::Relaxed);
                self.observer.update(Message::InternalError(err));
            }
        }
    }

    #[doc(hidden)]
    fn try_read_packet(&mut self) -> Result<(), ClientError> {
        let mut buf = [0u8; 1];

        match self.stream.read_exact(&mut buf) {
            Ok(()) => {
                self.handle_packet(buf[0])?;
                Ok(())
            }
            Err(err)
                if (err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::WouldBlock) =>
            {
                Ok(())
            }
            Err(err) => Err(ClientError::from(err)),
        }
    }

    #[doc(hidden)]
    fn handle_packet(&mut self, header: u8) -> Result<(), ClientError> {
        match get_code_type(header >> 4) {
            Ok(packet) => match packet {
                PacketType::Publish => self.handle_publish(header),
                PacketType::Puback => self.handle_puback(header),
                PacketType::Suback => self.handle_suback(header),
                PacketType::Unsuback => self.handle_unsuback(header),
                PacketType::Pingresp => self.handle_pingresp(header),
                PacketType::Connack => self.handle_connack(header),
                _ => Err(ClientError::new("Received an unsupported packet type")),
            },
            Err(error) => {
                self.observer
                    .update(Message::InternalError(ClientError::from(error)));
                Ok(())
            }
        }
    }

    #[doc(hidden)]
    fn handle_publish(&mut self, header: u8) -> Result<(), ClientError> {
        let publish = Publish::read_from(&mut self.stream, header)?;
        let id_opt = publish.packet_id().cloned();
        self.observer.update(Message::Publish(publish));

        // Si tiene id no es QoS 0
        if let Some(id) = id_opt {
            self.ack_sender.send_puback(Puback::new(id)?);
        }

        Ok(())
    }

    #[doc(hidden)]
    fn handle_connack(&mut self, header: u8) -> Result<(), ClientError> {
        let connack = Connack::read_from(&mut self.stream, header);

        let mut lock = self.pending_ack.lock()?;
        let expected = matches!(lock.as_ref(), Some(PendingAck::Connect(_)));
        match connack {
            Err(err) if !CONNECT_USER_ERRORS.contains(&err.kind()) => {
                return Err(ClientError::from(err));
            }
            Err(err) if expected => {
                // Si o si es uno de los CONNECT_USER_ERRORS
                lock.take();
                self.stop.store(true, Ordering::Relaxed);
                self.observer
                    .update(Message::Connected(Err(ClientError::from(err))));
            }
            Ok(packet) if expected => {
                lock.take();
                self.observer.update(Message::Connected(Ok(packet)));
            }
            _ => (),
        }

        Ok(())
    }

    #[doc(hidden)]
    fn handle_suback(&mut self, header: u8) -> Result<(), ClientError> {
        let suback = Suback::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::Subscribe(subscribe)) = lock.as_ref() {
            if subscribe.packet_identifier() == suback.packet_id() {
                lock.take();
                self.observer.update(Message::Subscribed(Ok(suback)));
            }
        }

        Ok(())
    }

    #[doc(hidden)]
    fn handle_puback(&mut self, header: u8) -> Result<(), ClientError> {
        let puback = Puback::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::Publish(publish)) = lock.as_ref() {
            if let Some(expected_id) = publish.packet_id() {
                if *expected_id == puback.packet_id() {
                    lock.take();
                    self.observer.update(Message::Published(Ok(Some(puback))));
                }
            } else {
                return Err(ClientError::new(
                    "The saved unacknowledged Publish packet has no id, so it can't have QoS 1",
                ));
            }
        }

        Ok(())
    }

    #[doc(hidden)]
    fn handle_pingresp(&mut self, header: u8) -> Result<(), ClientError> {
        let _ = PingResp::read_from(&mut self.stream, header)?;

        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::PingReq(_)) = lock.as_ref() {
            lock.take();
        }

        Ok(())
    }

    #[doc(hidden)]
    fn handle_unsuback(&mut self, header: u8) -> Result<(), ClientError> {
        let unsuback = Unsuback::read_from(&mut self.stream, header)?;
        let mut lock = self.pending_ack.lock()?;

        if let Some(PendingAck::Unsubscribe(unsubscribe)) = lock.as_ref() {
            if unsubscribe.packet_id() == unsuback.packet_id() {
                lock.take();
                self.observer.update(Message::Unsubscribed(Ok(unsuback)));
            }
        }

        Ok(())
    }
}

#[doc(hidden)]
fn get_code_type(code: u8) -> Result<PacketType, PacketError> {
    match code {
        1 => Ok(PacketType::Connect),
        2 => Ok(PacketType::Connack),
        3 => Ok(PacketType::Publish),
        4 => Ok(PacketType::Puback),
        8 => Ok(PacketType::Subscribe),
        9 => Ok(PacketType::Suback),
        10 => Ok(PacketType::Unsubscribe),
        11 => Ok(PacketType::Unsuback),
        12 => Ok(PacketType::Pingreq),
        13 => Ok(PacketType::Pingresp),
        14 => Ok(PacketType::Disconnect),
        _ => Err(PacketError::new_kind(
            &format!("Received invalid packet type: {}", code),
            ErrorKind::InvalidControlPacketType,
        )),
    }
}

#[cfg(test)]
mod tests {

    use std::io::{self, Cursor};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::client::PendingAck;
    use crate::client_packets::{ConnectBuilder, PingReq, Subscribe, Topic, Unsubscribe};
    use crate::observer::Message;
    use packets::packet_reader::QoSLevel::*;
    use packets::puback::Puback;
    use packets::publish::Publish;

    use super::{AckSender, ClientListener, ReadTimeout};

    #[derive(Clone)]
    struct ObserverMock {
        pub messages: Arc<Mutex<Vec<Message>>>,
    }

    impl super::Observer for ObserverMock {
        fn update(&self, message: super::Message) {
            self.messages.lock().unwrap().push(message);
        }
    }

    impl ObserverMock {
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl<T: AsRef<[u8]> + Send + Sync + 'static> ReadTimeout for Cursor<T> {
        fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
            Ok(())
        }
    }

    struct SenderMock {
        pub times_called: Mutex<u8>,
    }

    impl AckSender for SenderMock {
        fn send_puback(&self, _: Puback) {
            *self.times_called.lock().unwrap() += 1;
        }
    }

    impl SenderMock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                times_called: Mutex::new(0),
            })
        }
    }

    #[test]
    fn test_unsuback() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Unsubscribe(
            Unsubscribe::new(123, vec!["topic".to_string()]).unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b10110000, 2, 0, 123]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_none());
        let mut msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Unsubscribed(Ok(_))));
        if let Message::Unsubscribed(Ok(unsuback)) = msgs.remove(0) {
            assert_eq!(unsuback.packet_id(), 123);
        }
    }

    #[test]
    fn test_unsuback_different_id() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Unsubscribe(
            Unsubscribe::new(25, vec!["topic".to_string()]).unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b10110000, 2, 0, 123]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_some());
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Unsubscribed(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_unsuback_unexpected() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b10110000, 2, 0, 123]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::PingReq(_))
        ));
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Unsubscribed(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_suback() {
        let observer = ObserverMock::new();
        let topic = Topic::new("topic", QoSLevel1).unwrap();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Subscribe(Subscribe::new(
            vec![topic],
            123,
        )))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b10010000, 3, 0, 123, 0]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_none());
        let mut msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Subscribed(Ok(_))));
        if let Message::Subscribed(Ok(suback)) = msgs.remove(0) {
            assert_eq!(suback.packet_id(), 123);
        }
    }

    #[test]
    fn test_suback_different_id() {
        let observer = ObserverMock::new();
        let topic = Topic::new("topic", QoSLevel1).unwrap();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Subscribe(Subscribe::new(
            vec![topic],
            34,
        )))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b10010000, 3, 0, 123, 0]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_some());
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Subscribed(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_suback_unexpected() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b10010000, 3, 0, 123, 0]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::PingReq(_))
        ));
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Subscribed(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_puback() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Publish(
            Publish::new(false, QoSLevel1, false, "topic", "msg", Some(123)).unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(Puback::new(123).unwrap().encode());
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_none());
        let mut msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Published(Ok(Some(_)))));
        if let Message::Published(Ok(Some(puback))) = msgs.remove(0) {
            assert_eq!(puback.packet_id(), 123);
        }
    }

    #[test]
    fn test_puback_different_id() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Publish(
            Publish::new(false, QoSLevel1, false, "topic", "msg", Some(97)).unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(Puback::new(123).unwrap().encode());
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_some());
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Published(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_puback_unexpected() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(Puback::new(123).unwrap().encode());
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::PingReq(_))
        ));
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Published(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_puback_no_id_pending() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Publish(
            Publish::new(false, QoSLevel0, false, "topic", "msg", None).unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(Puback::new(123).unwrap().encode());
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_some());
        let msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::InternalError(_)));
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Published(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_pingresp() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b11010000, 0b00000000]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_none());
    }

    #[test]
    fn test_pingresp_unexpected() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Unsubscribe(
            Unsubscribe::new(25, vec!["topic".to_string()]).unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![0b11010000, 0b00000000]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::Unsubscribe(_))
        ));
    }

    #[test]
    fn test_publish_qos0() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let publish = Publish::new(false, QoSLevel0, false, "topic", "msg", None).unwrap();
        let stream = Cursor::new(publish.encode().unwrap());
        let sender = SenderMock::new();
        let mut listener = ClientListener::new(
            stream.clone(),
            pending_ack.clone(),
            observer.clone(),
            stop,
            sender.clone(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::PingReq(_))
        ));
        let mut msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Publish(_)));
        if let Message::Publish(publish) = msgs.remove(0) {
            assert_eq!(publish.packet_id(), None);
            assert_eq!(publish.topic_name(), "topic");
            assert_eq!(publish.payload(), Some(&"msg".to_string()));
        }
        assert_eq!(*sender.times_called.lock().unwrap(), 0);
    }

    #[test]
    fn test_publish_qos1() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let publish = Publish::new(false, QoSLevel1, false, "topic", "msg", Some(123)).unwrap();
        let stream = Cursor::new(publish.encode().unwrap());
        let sender = SenderMock::new();
        let mut listener = ClientListener::new(
            stream.clone(),
            pending_ack.clone(),
            observer.clone(),
            stop,
            sender.clone(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::PingReq(_))
        ));
        let mut msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Publish(_)));
        if let Message::Publish(publish) = msgs.remove(0) {
            assert_eq!(publish.packet_id(), Some(&123));
            assert_eq!(publish.topic_name(), "topic");
            assert_eq!(publish.payload(), Some(&"msg".to_string()));
        }
        assert_eq!(*sender.times_called.lock().unwrap(), 1);
    }

    #[test]
    fn test_connack() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Connect(
            ConnectBuilder::new("123", 0, true)
                .unwrap()
                .build()
                .unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![32, 2, 1, 0]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_none());
        let msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Connected(Ok(_))));
    }

    #[test]
    fn test_connack_unexpected() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::PingReq(PingReq::new()))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![32, 2, 1, 0]);
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::PingReq(_))
        ));
        let msgs = observer.messages.lock().unwrap();
        let i = msgs
            .iter()
            .position(|packet| matches!(packet, Message::Connected(Ok(_))));
        assert!(i.is_none());
    }

    #[test]
    fn test_connack_user_error() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Connect(
            ConnectBuilder::new("123", 0, true)
                .unwrap()
                .build()
                .unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![32, 2, 1, 5]); // no autorizado
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(pending_ack.lock().unwrap().is_none());
        let msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::Connected(Err(_))));
    }

    #[test]
    fn test_connack_another_error() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(Some(PendingAck::Connect(
            ConnectBuilder::new("123", 0, true)
                .unwrap()
                .build()
                .unwrap(),
        ))));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![33, 2, 1, 0]); // mal los bits reservados
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        assert!(matches!(
            *pending_ack.lock().unwrap(),
            Some(PendingAck::Connect(_))
        ));
        let msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::InternalError(_)));
    }

    #[test]
    fn test_invalid_packet() {
        let observer = ObserverMock::new();
        let pending_ack = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let stream = Cursor::new(vec![224, 2, 1, 5]); // header de disconnect
        let mut listener = ClientListener::new(
            stream,
            pending_ack.clone(),
            observer.clone(),
            stop,
            SenderMock::new(),
        )
        .unwrap();
        listener.wait_for_packets();

        let msgs = observer.messages.lock().unwrap();
        assert!(matches!(msgs[0], Message::InternalError(_)));
    }
}
