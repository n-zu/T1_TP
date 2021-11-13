use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::{io, thread};
use std::{net::TcpStream, time::Duration};

pub mod client_error;
mod client_listener;
mod client_sender;

use crate::client_packets::unsubscribe::Unsubscribe;
use crate::client_packets::{Connect, PingReq, Subscribe};
use client_listener::ClientListener;
use client_sender::ClientSender;

use crate::observer::{Message, Observer};
pub use client_error::ClientError;
use packets::publish::Publish;
use threadpool::ThreadPool;

use self::client_listener::Stream;

/// Enum for Pending Acknowledgments of sent packets
/// Common interface for the listener and the sender
#[derive(Debug)]
pub(crate) enum PendingAck {
    Subscribe(Subscribe),
    Unsubscribe(Unsubscribe),
    PingReq(PingReq),
    Publish(Publish),
    Connect(Connect),
}

/// Internal Client. Fully functional MQTT Client
/// which lacks any I/O to the user
pub struct Client<T: Observer> {
    thread_pool: ThreadPool,
    stop: Arc<AtomicBool>,
    sender: Arc<ClientSender<T, TcpStream>>,
}

impl Stream for TcpStream {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }
}

/// How often should the listener and ping sender check to see if they should stop
pub(crate) const STOP_TIMEOUT: Duration = Duration::from_millis(200);

/// How much to reduce from the given Keep Alive time in orden to have an error margin
pub(crate) const KEEP_ALIVE_SUBSTRACTION: Duration = Duration::from_secs(2);

impl<T: Observer> Client<T> {
    #![allow(dead_code)]
    /// Creates a new Client which connects to the TCP Listener on the given address, by
    /// sending the given CONNECT packet.
    /// The client must be initialized with an Observer to receive the different
    /// Messages the client sends after relevant events (defined in the trait Observer).
    /// If the connect packet has a Keep Alive set, it will automatically send and receive
    /// the PingReq and PingResp packets
    pub fn new(address: &str, observer: T, connect: Connect) -> Result<Client<T>, ClientError> {
        let stream = TcpStream::connect(address)?;
        let mut threads = 3;
        let keep_alive = connect.keep_alive();
        if keep_alive == 0 {
            threads = 2; // no lo necesito para el pingreq
        }

        let mut ret = Client {
            thread_pool: ThreadPool::new(threads), // Me aseguro que solo este el listener y 1 sender
            stop: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(ClientSender::new(stream, observer)),
        };

        ret.connect(connect)?;

        ret.setup_keep_alive(keep_alive)?;

        Ok(ret)
    }

    #[doc(hidden)]
    fn setup_keep_alive(&self, seconds: u16) -> Result<(), ClientError> {
        if seconds == 0 {
            return Ok(());
        }
        let duration = Duration::from_secs(seconds.into());
        let sender = self.sender.clone();
        let stop = self.stop.clone();

        self.thread_pool.spawn(move || {
            Self::keep_alive(sender, stop, duration);
        })?;

        Ok(())
    }

    #[doc(hidden)]
    fn keep_alive(
        sender: Arc<ClientSender<T, TcpStream>>,
        stop: Arc<AtomicBool>,
        mut duration: Duration,
    ) {
        let mut now = std::time::Instant::now();
        if duration > KEEP_ALIVE_SUBSTRACTION {
            duration -= KEEP_ALIVE_SUBSTRACTION;
        }

        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            thread::sleep(STOP_TIMEOUT);
            if now.elapsed() > duration {
                now = std::time::Instant::now();
                sender.send_pingreq();
            }
        }
    }

    #[doc(hidden)]
    fn connect(&mut self, connect: Connect) -> Result<(), ClientError> {
        let read_stream = self.sender.stream().lock()?.try_clone()?;
        let mut listener = ClientListener::new(
            read_stream,
            self.sender.pending_ack(),
            self.sender.observer(),
            self.stop.clone(),
        )?;

        let sender = self.sender.clone();
        let stop = self.stop.clone();
        self.thread_pool.spawn(move || {
            sender.send_connect(connect, stop);
        })?;

        self.thread_pool.spawn(move || {
            listener.wait_for_packets();
        })?;

        Ok(())
    }

    /// Sends the given SUBSCRIBE packet to the server. The Client then either returns
    /// Err(ClientError) or Ok(()). In the latter case, the result of the operation
    /// is sent to the Observer with a Subscribed() message.
    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), ClientError> {
        let sender = self.sender.clone();

        self.thread_pool.spawn(move || {
            sender.send_subscribe(subscribe);
        })?;

        Ok(())
    }

    /// Sends the given UNSUBSCRIBE packet to the server. The Client then either returns
    /// Err(ClientError) or Ok(()). In the latter case, the result of the operation
    /// is sent to the Observer with a Unsubscribed() message.
    pub fn unsubscribe(&mut self, unsubscribe: Unsubscribe) -> Result<(), ClientError> {
        let sender = self.sender.clone();

        self.thread_pool.spawn(move || {
            sender.send_unsubscribe(unsubscribe);
        })?;

        Ok(())
    }

    /// Sends the given publish packet to the server. The Client then either returns
    /// Err(ClientError) or Ok(()). In the latter case, the result of the operation
    /// is sent to the Observer with a Published() message. If the QoS of the packet
    /// is QoSLevel1, not receiving the corresponding PUBACK packet will result in an
    /// Error. If it succeeds, it sends a Published(Ok(None)) message if the packet
    /// had QoSLevel0 or Published(Ok(Some())) with the corresponding PUBACK if the
    /// packet had QoSLevel1. Behaviour is undefined for QoSLevel2.
    pub fn publish(&mut self, publish: Publish) -> Result<(), ClientError> {
        let sender = self.sender.clone();
        self.thread_pool.spawn(move || {
            sender.send_publish(publish);
        })?;

        Ok(())
    }
}

impl<T: Observer> Drop for Client<T> {
    /// The client automatically sends a disconnect packet before dropping and closing the connection.
    /// If this fails, an InternalError is sent to the observer but the connection is closed anyway.
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let sender = self.sender.clone();
        if let Err(err) = self.thread_pool.spawn(move || {
            sender.send_disconnect();
        }) {
            let msg = "Error enviándo paquete disconnect, se desconectará de manera forzosa";
            let error = ClientError::new(&format!("{}\n{}", msg, err));
            self.sender.observer().update(Message::InternalError(error));
        }
    }
}
