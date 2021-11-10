use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::{net::TcpStream, time::Duration};

pub mod client_error;
mod client_listener;
mod client_sender;

use crate::client_packets::{Connect, PingReq, Subscribe};
use client_listener::Listener;
use client_sender::ClientSender;

use crate::observer::Observer;
pub use client_error::ClientError;
use packets::publish::Publish;
use threadpool::ThreadPool;

#[allow(dead_code)]
pub enum PendingAck {
    Subscribe(Subscribe),
    PingReq(PingReq),
    Publish(Publish),
    Connect(Connect),
}

pub struct Client<T: Observer> {
    thread_pool: ThreadPool,
    stop: Arc<AtomicBool>,
    sender: Arc<ClientSender<T>>,
}

// Cuanto esperar antes de checkear si hay que parar
const STOP_TIMEOUT: Duration = Duration::from_millis(200);

// Cuanto restarle al Keep Alive como márgen de error
const KEEP_ALIVE_SUBSTRACTION: Duration = Duration::from_secs(2);

impl<T: 'static + Observer> Client<T> {
    #![allow(dead_code)]
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

    fn keep_alive(sender: Arc<ClientSender<T>>, stop: Arc<AtomicBool>, mut duration: Duration) {
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

    fn connect(&mut self, connect: Connect) -> Result<(), ClientError> {
        let read_stream = self.sender.stream().lock()?.try_clone()?;
        let mut listener = Listener::new(
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

    pub fn subscribe(&mut self, subscribe: Subscribe) -> Result<(), ClientError> {
        let sender = self.sender.clone();

        self.thread_pool.spawn(move || {
            sender.send_subscribe(subscribe);
        })?;

        Ok(())
    }

    pub fn publish(&mut self, publish: Publish) -> Result<(), ClientError> {
        let sender = self.sender.clone();
        self.thread_pool.spawn(move || {
            sender.send_publish(publish);
        })?;

        Ok(())
    }
}

impl<T: Observer> Drop for Client<T> {
    fn drop(&mut self) {
        println!("Cliente dropeado");
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
