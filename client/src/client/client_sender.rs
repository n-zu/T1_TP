use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time};

use packets::packet_reader::QoSLevel;

use crate::client_packets::unsubscribe::Unsubscribe;
use crate::client_packets::{Connect, Disconnect, PingReq, Subscribe};
use crate::observer::{Message, Observer};
use packets::publish::Publish;

use super::{ClientError, PendingAck};

/// How much time should the sender wait until it tries
/// to resend an unacknowledged packet.
pub(crate) const RESEND_TIMEOUT: Duration = Duration::from_millis(5000);

/// How often should the sender check pending_ack after
/// sending a packet that needs acknowledgement to see
/// if it was acknowledged.
pub(crate) const ACK_CHECK: Duration = Duration::from_millis(500);

/// The maximum number of times the sender should try to
/// resend an unacknowledged packet.
pub(crate) const MAX_RETRIES: u16 = 3;

/// The packet sender of the client. It is responsible
/// for sending all packets to the server, except for
/// acknowledgements.
pub(crate) struct ClientSender<T: Observer, W: Write> {
    stream: Mutex<W>,
    pending_ack: Arc<Mutex<Option<PendingAck>>>,
    observer: Arc<T>,
}

impl<T: Observer, W: Write> ClientSender<T, W> {
    /// Creates a new sender with the given stream and observer,
    /// and intializes pending_ack to None.
    pub fn new(stream: W, observer: T) -> Self {
        Self {
            stream: Mutex::new(stream),
            pending_ack: Arc::new(Mutex::new(None)),
            observer: Arc::new(observer),
        }
    }

    /// Gets the pending_ack lock of the sender. This is used
    /// by the sender after sending a packet to check if it was
    /// acknowledged. If it was, it expects the lock to be
    /// set to None.
    pub fn pending_ack(&self) -> Arc<Mutex<Option<PendingAck>>> {
        self.pending_ack.clone()
    }

    /// Gets the stream that the sender is using.
    pub fn stream(&self) -> &Mutex<W> {
        &self.stream
    }

    /// Gets a clone of the observer of the sender.
    pub fn observer(&self) -> Arc<T> {
        self.observer.clone()
    }

    #[doc(hidden)]
    fn _connect(&self, connect: Connect) -> Result<(), ClientError> {
        let bytes = connect.encode();
        self.pending_ack
            .lock()?
            .replace(PendingAck::Connect(connect));
        self.stream.lock()?.write_all(&bytes)?;

        if !self.wait_for_ack()? {
            return Err(ClientError::new("No se pudo establecer la conexión"));
        }

        Ok(())
    }

    /// Sends a CONNECT packet to the server.
    /// After sending the packet, it will set pending_ack to PendingAck::Connect()
    /// and will wait until it is None. Every time a RESEND_TIMEOUT duration passes,
    /// it will resend the package up to a maximum of MAX_RETRIES times, after which
    /// it fails.
    /// If it fails, it sets failure_stop to true and sends a Message::Conected
    /// with the error to the observer and pending_ack is set to None.
    pub fn send_connect(&self, connect: Connect, failure_stop: Arc<AtomicBool>) {
        if let Err(err) = self._connect(connect) {
            failure_stop.store(true, std::sync::atomic::Ordering::Relaxed);
            self.observer.update(Message::Connected(Err(err)));
        }
    }

    #[doc(hidden)]
    fn _subscribe(&self, subscribe: Subscribe) -> Result<(), ClientError> {
        let bytes = subscribe.encode()?;
        self.pending_ack
            .lock()?
            .replace(PendingAck::Subscribe(subscribe));
        self.stream.lock()?.write_all(&bytes)?;
        if !self.wait_for_ack()? {
            return Err(ClientError::new("No se recibió paquete suback"));
        }

        Ok(())
    }

    /// Sends a SUBSCRIBE packet to the server.
    /// After sending the packet, it will set pending_ack to PendingAck::Subscribe()
    /// and will wait until it is None. Every time a RESEND_TIMEOUT duration passes,
    /// it will resend the package up to a maximum of MAX_RETRIES times, after which
    /// it fails.
    /// If it fails, it sends a Message::Subscribed with the error to the observer
    /// and pending_ack is set to None.
    pub fn send_subscribe(&self, subscribe: Subscribe) {
        if let Err(err) = self._subscribe(subscribe) {
            self.observer.update(Message::Subscribed(Err(err)));
        }
    }

    pub fn _publish(&self, publish: Publish) -> Result<(), ClientError> {
        let bytes = publish.encode()?;
        let qos = publish.qos();
        if qos == QoSLevel::QoSLevel1 {
            *self.pending_ack.lock()? = Some(PendingAck::Publish(publish));
        }

        self.stream.lock()?.write_all(&bytes)?;

        if qos == QoSLevel::QoSLevel1 {
            if !self.wait_for_ack()? {
                return Err(ClientError::new("No se recibió paquete puback (QoS 1)"));
            }
        } else {
            self.observer.update(Message::Published(Ok(None)));
        }

        Ok(())
    }

    /// Sends a PUBLISH packet to the server.
    ///
    /// If the packet has QoSLevel 1:
    /// After sending the packet, it will set pending_ack to PendingAck::Publish() and
    /// will wait until it is None. Every time a RESEND_TIMEOUT duration passes, it will
    /// resend the package with the DUP flag set up to a maximum of MAX_RETRIES times,
    /// after which it fails.
    ///
    /// If the packet has QoSLevel 0:
    /// After sending the packet, it will send a Message::Published with Ok() to the
    /// observer.
    ///
    /// If the packet has QoSLevel 2, the behaviour is undefined.
    ///
    /// If it fails, it sends a Message::Published with the error to the observer
    /// (and pending_ack is set to None if QoSLevel was 1)
    pub fn send_publish(&self, publish: Publish) {
        if let Err(err) = self._publish(publish) {
            self.observer.update(Message::Published(Err(err)));
        }
    }

    #[doc(hidden)]
    fn _pingreq(&self, pingreq: PingReq) -> Result<(), ClientError> {
        let bytes = pingreq.encode();
        self.pending_ack
            .lock()?
            .replace(PendingAck::PingReq(pingreq));
        self.stream.lock()?.write_all(&bytes)?;
        if !self.wait_for_ack()? {
            return Err(ClientError::new(
                "El servidor no respondió al pingreq, ¿esta en línea?",
            ));
        }

        Ok(())
    }

    /// Sends a PINGREQ packet to the server.
    /// After sending the packet, it will set pending_ack to PendingAck::PingReq()
    /// and will wait until it is None. Every time a RESEND_TIMEOUT duration passes,
    /// it will resend the package up to a maximum of MAX_RETRIES times, after which
    /// it fails.
    /// If it fails, it sends a Message::InternalError() with the error to the observer
    /// and pending_ack is set to None.
    pub fn send_pingreq(&self) {
        let pingreq = PingReq::new();
        if let Err(err) = self._pingreq(pingreq) {
            self.observer.update(Message::InternalError(err));
        }
    }

    #[doc(hidden)]
    fn _disconnect(&self, disconnect: Disconnect) -> Result<(), ClientError> {
        self.stream.lock()?.write_all(&disconnect.encode())?;
        Ok(())
    }

    /// Sends a DISCONNECT packet to the server.
    /// If it fails, it sends a Message::InternalError with the error to the observer
    pub fn send_disconnect(&self) {
        let disconnect = Disconnect::new();
        if let Err(err) = self._disconnect(disconnect) {
            self.observer.update(Message::InternalError(err));
        }
    }

    #[doc(hidden)]
    fn _unsubscribe(&self, unsubscribe: Unsubscribe) -> Result<(), ClientError> {
        let bytes = unsubscribe.encode()?;
        self.pending_ack
            .lock()?
            .replace(PendingAck::Unsubscribe(unsubscribe));
        self.stream.lock()?.write_all(&bytes)?;
        if !self.wait_for_ack()? {
            return Err(ClientError::new("No se recibió paquete unsuback"));
        }

        Ok(())
    }

    /// Sends an UNSUBSCRIBE packet to the server.
    /// After sending the packet, it will set pending_ack to PendingAck::Unsubscribe()
    /// and will wait until it is None. Every time a RESEND_TIMEOUT duration passes,
    /// it will resend the package up to a maximum of MAX_RETRIES times, after which
    /// it fails.
    /// If it fails, it sends a Message::Unsubscribed with the error to the observer
    /// and pending_ack is set to None.
    pub fn send_unsubscribe(&self, unsubscribe: Unsubscribe) {
        if let Err(err) = self._unsubscribe(unsubscribe) {
            self.observer.update(Message::Unsubscribed(Err(err)));
        }
    }

    #[doc(hidden)]
    fn resend_pending(stream: &mut W, pending_ack: &mut PendingAck) -> Result<(), ClientError> {
        // TODO: realmente deberiamos hacer de una vez el trait de encode asi evitamos esto
        match pending_ack {
            PendingAck::Subscribe(subscribe) => {
                stream.write_all(&subscribe.encode()?)?;
            }
            PendingAck::PingReq(ping_req) => {
                stream.write_all(&ping_req.encode())?;
            }
            PendingAck::Publish(publish) => {
                publish.set_dup(true);
                stream.write_all(&publish.encode()?)?;
            }
            PendingAck::Connect(connect) => {
                stream.write_all(&connect.encode())?;
            }
            PendingAck::Unsubscribe(unsubscribe) => {
                stream.write_all(&unsubscribe.encode()?)?;
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    // Devuelve verdadero si se pudo mandar, falso si no se recibió el ack
    fn wait_for_ack(&self) -> Result<bool, ClientError> {
        let mut retries = 0;
        let mut stream = self.stream.lock()?;
        let mut last = time::Instant::now();

        while retries < MAX_RETRIES {
            thread::sleep(ACK_CHECK);
            match self.pending_ack.lock()?.as_mut() {
                None => {
                    return Ok(true);
                }
                Some(pending_ack) => {
                    let now = time::Instant::now();
                    if last + RESEND_TIMEOUT < now {
                        Self::resend_pending(&mut stream, pending_ack)?;
                        last = time::Instant::now();
                        retries += 1;
                    }
                }
            }
        }

        self.pending_ack.lock()?.take();
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor as IoCursor, Write},
        sync::{atomic::AtomicBool, Arc, Mutex},
        thread,
        time::Instant,
    };

    use packets::{packet_reader::QoSLevel, publish::Publish};

    use crate::{
        client::{client_sender::MAX_RETRIES, PendingAck},
        client_packets::{
            subscribe::{Subscribe, Topic},
            unsubscribe::Unsubscribe,
            ConnectBuilder, Disconnect, PingReq,
        },
        observer::Message,
    };

    use super::ClientSender;

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

    #[derive(Clone)]
    struct Cursor {
        cursor: Arc<Mutex<IoCursor<Vec<u8>>>>,
    }

    impl Cursor {
        fn new() -> Self {
            Self {
                cursor: Arc::new(Mutex::new(IoCursor::new(Vec::new()))),
            }
        }

        fn content(&self) -> Vec<u8> {
            self.cursor.lock().unwrap().clone().into_inner()
        }
    }

    impl Write for Cursor {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.cursor.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.cursor.lock().unwrap().flush()
        }
    }

    fn take_ack(client_sender: &ClientSender<ObserverMock, Cursor>) -> Option<PendingAck> {
        let pending = client_sender.pending_ack();

        let max = Instant::now() + std::time::Duration::from_millis(5000);
        while max > Instant::now() {
            thread::sleep(std::time::Duration::from_millis(200));
            let mut lock = pending.lock().unwrap();
            if lock.is_some() {
                let msg = lock.take();
                return msg;
            }
        }
        None
    }

    #[test]
    fn test_connect() {
        let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
        let bytes = connect.encode();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));
        let pending = client_sender.pending_ack();
        let stop = Arc::new(AtomicBool::new(false));

        let client_sender_clone = client_sender.clone();
        let stop_clone = stop.clone();
        let handle = thread::spawn(move || {
            client_sender_clone.send_connect(connect, stop_clone);
        });

        assert!(matches!(
            take_ack(&client_sender),
            Some(PendingAck::Connect(_))
        ));
        // Debería haber puesto en el pending_ack un PendingAck::Connect()

        handle.join().unwrap();

        assert!(pending.lock().unwrap().is_none());
        // Una vez terminó y ya habiendo otro thread sacado el PendingAck, debería
        // haber quedado en None

        assert!(!stop.load(std::sync::atomic::Ordering::Relaxed));
        // No falló asique no tiene por que cambiar el stop

        assert_eq!(stream.content(), bytes);
        // Debería haber escrito el connect en el stream

        assert!(observer.messages.lock().unwrap().is_empty());
        // No le debería haber mandado nada al observer
    }

    #[test]
    fn test_connect_fail() {
        let connect = ConnectBuilder::new("id", 0, true).unwrap().build().unwrap();
        let bytes = connect.encode();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(stream.clone(), observer.clone());
        let pending = client_sender.pending_ack();
        let stop = Arc::new(AtomicBool::new(false));

        let stop_clone = stop.clone();

        client_sender.send_connect(connect, stop_clone);
        // No le saco el pending_ack()

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        assert!(stop.load(std::sync::atomic::Ordering::Relaxed));
        // Falló asi que tiene que cambiar el stop

        assert_eq!(stream.content(), bytes.repeat(1 + MAX_RETRIES as usize));
        // Debería haber escrito el connect en el stream, con el intento inicial + MAX_RETRIES veces

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Connected(Err(_))
        ));
        // Debería haber mandado el error al observer
    }

    #[test]
    fn test_subscribe() {
        let topic = Topic::new("cars/wheels", QoSLevel::QoSLevel0).unwrap();
        let subscribe = Subscribe::new(vec![topic], 123);
        let bytes = subscribe.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));
        let pending = client_sender.pending_ack();

        let client_sender_clone = client_sender.clone();
        let handle = thread::spawn(move || {
            client_sender_clone.send_subscribe(subscribe);
        });

        assert!(matches!(
            take_ack(&client_sender),
            Some(PendingAck::Subscribe(_))
        ));
        // Debería haber puesto en el pending_ack un PendingAck::Subscribe()

        handle.join().unwrap();

        assert!(pending.lock().unwrap().is_none());
        // Una vez terminó y ya habiendo otro thread sacado el PendingAck, debería
        // haber quedado en None

        assert_eq!(stream.content(), bytes);
        // Debería haber escrito el subscribe en el stream

        assert!(observer.messages.lock().unwrap().is_empty());
        // No le debería haber mandado nada al observer
    }

    #[test]
    fn test_subscribe_fail() {
        let topic = Topic::new("cars/wheels", QoSLevel::QoSLevel0).unwrap();
        let subscribe = Subscribe::new(vec![topic], 123);
        let bytes = subscribe.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(stream.clone(), observer.clone());
        let pending = client_sender.pending_ack();

        client_sender.send_subscribe(subscribe);

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        assert_eq!(stream.content(), bytes.repeat(1 + MAX_RETRIES as usize));
        // Debería haber escrito el subscribe en el stream, con el intento inicial + MAX_RETRIES veces

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Subscribed(Err(_))
        ));
        // Debería haber mandado el error al observer
    }

    #[test]
    fn test_unsubscribe() {
        let unsubscribe = Unsubscribe::new(123, vec!["car/wheels".to_string()]).unwrap();
        let bytes = unsubscribe.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));
        let pending = client_sender.pending_ack();

        let client_sender_clone = client_sender.clone();
        let handle = thread::spawn(move || {
            client_sender_clone.send_unsubscribe(unsubscribe);
        });

        assert!(matches!(
            take_ack(&client_sender),
            Some(PendingAck::Unsubscribe(_))
        ));
        // Debería haber puesto en el pending_ack un PendingAck::Unsubscribe()

        handle.join().unwrap();

        assert!(pending.lock().unwrap().is_none());
        // Una vez terminó y ya habiendo otro thread sacado el PendingAck, debería
        // haber quedado en None

        assert_eq!(stream.content(), bytes);
        // Debería haber escrito el subscribe en el stream

        assert!(observer.messages.lock().unwrap().is_empty());
        // No le debería haber mandado nada al observer
    }

    #[test]
    fn test_unsubscribe_fail() {
        let unsubscribe = Unsubscribe::new(123, vec!["car/wheels".to_string()]).unwrap();
        let bytes = unsubscribe.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(stream.clone(), observer.clone());
        let pending = client_sender.pending_ack();

        client_sender.send_unsubscribe(unsubscribe);

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        assert_eq!(stream.content(), bytes.repeat(1 + MAX_RETRIES as usize));
        // Debería haber escrito el unsubscribe en el stream, con el intento inicial + MAX_RETRIES veces

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Unsubscribed(Err(_))
        ));
        // Debería haber mandado el error al observer
    }

    #[test]
    fn test_pingreq() {
        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));
        let pending = client_sender.pending_ack();

        let client_sender_clone = client_sender.clone();
        let handle = thread::spawn(move || {
            client_sender_clone.send_pingreq();
        });

        assert!(matches!(
            take_ack(&client_sender),
            Some(PendingAck::PingReq(_))
        ));
        // Debería haber puesto en el pending_ack un PendingAck::PingReq()

        handle.join().unwrap();

        assert!(pending.lock().unwrap().is_none());
        // Una vez terminó y ya habiendo otro thread sacado el PendingAck, debería
        // haber quedado en None

        assert_eq!(stream.content(), PingReq::new().encode());
        // Debería haber escrito el pingreq en el stream

        assert!(observer.messages.lock().unwrap().is_empty());
        // No le debería haber mandado nada al observer
    }

    #[test]
    fn test_pingreq_fail() {
        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(stream.clone(), observer.clone());
        let pending = client_sender.pending_ack();

        client_sender.send_pingreq();

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        assert_eq!(
            stream.content(),
            PingReq::new().encode().repeat(1 + MAX_RETRIES as usize)
        );
        // Debería haber escrito el unsubscribe en el stream, con el intento inicial + MAX_RETRIES veces

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::InternalError(_)
        ));
        // Debería haber mandado el error al observer
    }

    #[test]
    fn test_disconnect() {
        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));

        client_sender.send_disconnect();

        assert_eq!(stream.content(), Disconnect::new().encode());
        // Debería haber escrito el disconnect en el stream

        assert!(observer.messages.lock().unwrap().is_empty());
        // No le debería haber mandado nada al observer
    }

    struct BadWriter;
    impl Write for BadWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "can't write",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "can't flush",
            ))
        }
    }

    #[test]
    fn test_disconnect_fail() {
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(BadWriter {}, observer.clone()));

        client_sender.send_disconnect();

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::InternalError(_)
        ));
        // Debería haber mandado el error al observer
    }

    #[test]
    fn test_publish_qos1() {
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "car/wheels",
            "wow such wheel",
            Some(123),
        )
        .unwrap();
        let bytes = publish.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));
        let pending = client_sender.pending_ack();

        let client_sender_clone = client_sender.clone();
        let handle = thread::spawn(move || {
            client_sender_clone.send_publish(publish);
        });

        assert!(matches!(
            take_ack(&client_sender),
            Some(PendingAck::Publish(_))
        ));
        // Debería haber puesto en el pending_ack un PendingAck::Publish()

        handle.join().unwrap();

        assert!(pending.lock().unwrap().is_none());
        // Una vez terminó y ya habiendo otro thread sacado el PendingAck, debería
        // haber quedado en None

        assert_eq!(stream.content(), bytes);
        // Debería haber escrito el publish en el stream

        assert!(observer.messages.lock().unwrap().is_empty());
        // No le debería haber mandado nada al observer
    }

    #[test]
    fn test_publish_qos1_fail() {
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "car/wheels",
            "wow such wheel",
            Some(123),
        )
        .unwrap();
        let mut bytes = publish.encode().unwrap();
        let mut pub_dup = publish.clone();
        pub_dup.set_dup(true);
        let dup_bytes = pub_dup.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(stream.clone(), observer.clone());
        let pending = client_sender.pending_ack();

        client_sender.send_publish(publish);

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        println!("{:?}", bytes);
        println!("{:?}", dup_bytes);
        bytes.append(&mut dup_bytes.repeat(MAX_RETRIES as usize));
        assert_eq!(stream.content(), bytes);
        // Debería haber escrito el unsubscribe en el stream, con el intento inicial + MAX_RETRIES veces,
        // y los ultimos deberían tener la flag de DUP

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Published(Err(_))
        ));
        // Debería haber mandado el error al observer
    }

    #[test]
    fn test_publish_qos0() {
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel0,
            false,
            "car/wheels",
            "wow such wheel",
            None,
        )
        .unwrap();
        let bytes = publish.encode().unwrap();

        let stream = Cursor::new();
        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(stream.clone(), observer.clone());

        client_sender.send_publish(publish);

        assert_eq!(stream.content(), bytes);
        // Debería haber escrito el publish en el stream

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Published(Ok(_))
        ));
        // Le debería haber mandado el Ok al observer
    }

    #[test]
    fn test_publish_qos0_fail() {
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel0,
            false,
            "car/wheels",
            "wow such wheel",
            None,
        )
        .unwrap();

        let observer = ObserverMock::new();

        let client_sender = ClientSender::new(BadWriter {}, observer.clone());

        client_sender.send_publish(publish);

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Published(Err(_))
        ));
        // Debería haber mandado el error al observer
    }
}
