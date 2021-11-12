use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time};

use packets::packet_reader::QoSLevel;

use crate::client_packets::unsubscribe::Unsubscribe;
use crate::client_packets::{Connect, Disconnect, PingReq, Subscribe};
use crate::observer::{Message, Observer};
use packets::publish::{DupFlag, Publish};

use super::{ClientError, PendingAck};

// Cuanto esperar antes de checkear que
// haya que reenviar un paquete con QoS 1
const RESEND_TIMEOUT: Duration = Duration::from_millis(5000);

// Cada cuanto checkear si el listener ya recibió el ACK
const ACK_CHECK: Duration = Duration::from_millis(500);

// Cuantas veces reintentar reenviar un paquete
const MAX_RETRIES: u16 = 3;

pub struct ClientSender<T: Observer, W: Write> {
    stream: Mutex<W>,
    pending_ack: Arc<Mutex<Option<PendingAck>>>,
    observer: Arc<T>,
}

impl<T: Observer, W: Write> ClientSender<T, W> {
    #![allow(dead_code)]
    pub fn new(stream: W, observer: T) -> Self {
        Self {
            stream: Mutex::new(stream),
            pending_ack: Arc::new(Mutex::new(None)),
            observer: Arc::new(observer),
        }
    }

    pub fn pending_ack(&self) -> Arc<Mutex<Option<PendingAck>>> {
        self.pending_ack.clone()
    }

    pub fn stream(&self) -> &Mutex<W> {
        &self.stream
    }

    pub fn observer(&self) -> Arc<T> {
        self.observer.clone()
    }

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
    /// and will wait until it is None. Every time a RESEND_TIMEOUT passes, it will
    /// resend the package up to a maximum of MAX_RETRIES times, in which case
    /// it fails.
    /// If it fails, it sets failure_stop to true and sends a Message::Conected
    /// with the error to the observer. pending_ack is set to None.
    pub fn send_connect(&self, connect: Connect, failure_stop: Arc<AtomicBool>) {
        if let Err(err) = self._connect(connect) {
            failure_stop.store(true, std::sync::atomic::Ordering::Relaxed);
            self.observer.update(Message::Connected(Err(err)));
        }
    }

    pub fn _subscribe(&self, subscribe: Subscribe) -> Result<(), ClientError> {
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

    pub fn send_subscribe(&self, subscribe: Subscribe) {
        if let Err(err) = self._subscribe(subscribe) {
            println!("sending error");
            self.observer.update(Message::Subscribed(Err(err)));
        }
    }

    pub fn _publish(&self, publish: Publish) -> Result<(), ClientError> {
        let bytes = publish.encode()?;
        let qos = publish.qos();
        if qos == QoSLevel::QoSLevel1 {
            *self.pending_ack.lock()? = Some(PendingAck::Publish(publish));
            if !self.wait_for_ack()? {
                return Err(ClientError::new("No se recibió paquete puback (QoS 1)"));
            }
        }
        self.stream.lock()?.write_all(&bytes)?;

        if qos == QoSLevel::QoSLevel0 {
            self.observer.update(Message::Published(Ok(None)));
        }
        Ok(())
    }

    pub fn send_publish(&self, publish: Publish) {
        if let Err(err) = self._publish(publish) {
            self.observer.update(Message::Published(Err(err)));
        }
    }

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

    pub fn send_pingreq(&self) {
        let pingreq = PingReq::new();
        if let Err(err) = self._pingreq(pingreq) {
            self.observer.update(Message::InternalError(err));
        }
    }

    fn _disconnect(&self, disconnect: Disconnect) -> Result<(), ClientError> {
        self.stream.lock()?.write_all(&disconnect.encode())?;
        Ok(())
    }

    pub fn send_disconnect(&self) {
        let disconnect = Disconnect::new();
        if let Err(err) = self._disconnect(disconnect) {
            self.observer.update(Message::InternalError(err));
        }
    }

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

    pub fn send_unsubscribe(&self, unsubscribe: Unsubscribe) {
        if let Err(err) = self._unsubscribe(unsubscribe) {
            self.observer.update(Message::Unsubscribed(Err(err)));
        }
    }

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
                publish.set_dup_flag(DupFlag::DupFlag1);
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

    use crate::{
        client::{client_sender::MAX_RETRIES, PendingAck},
        client_packets::ConnectBuilder,
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
        fn new(data: Vec<u8>) -> Self {
            Self {
                cursor: Arc::new(Mutex::new(IoCursor::new(data))),
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

        let stream = Cursor::new(Vec::new());
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

        let stream = Cursor::new(Vec::new());
        let observer = ObserverMock::new();

        let client_sender = Arc::new(ClientSender::new(stream.clone(), observer.clone()));
        let pending = client_sender.pending_ack();
        let stop = Arc::new(AtomicBool::new(false));

        let client_sender_clone = client_sender.clone();
        let stop_clone = stop.clone();

        client_sender_clone.send_connect(connect, stop_clone);
        // No le saco el pending_ack()

        assert!(pending.lock().unwrap().is_none());
        // Al fallar, tiene que dejar vacío el pending_ack

        assert!(stop.load(std::sync::atomic::Ordering::Relaxed));
        // Falló asique tiene que cambiar el stop

        assert_eq!(stream.content(), bytes.repeat(1 + MAX_RETRIES as usize));
        // Debería haber escrito el connect en el stream, con el intento inicial + MAX_RETRIES veces

        assert!(matches!(
            observer.messages.lock().unwrap()[0],
            Message::Connected(Err(_))
        ));
        // Debería haber mandado el error al observer
    }
}
