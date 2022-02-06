use std::{
    error::Error,
    sync::{mpsc::Sender, Arc, Mutex},
};

use mqtt_client::{Message, Observer};

#[derive(Clone)]
pub struct ThermometerObserver {
    sender: Arc<Mutex<Sender<Message>>>,
}

impl ThermometerObserver {
    pub fn new(channel: Sender<Message>) -> ThermometerObserver {
        ThermometerObserver {
            sender: Arc::new(Mutex::new(channel)),
        }
    }

    fn try_send(&self, msg: Message) -> Result<(), Box<dyn Error + '_>> {
        self.sender.lock()?.send(msg)?;
        Ok(())
    }
}

impl Observer for ThermometerObserver {
    fn update(&self, msg: Message) {
        if let Err(e) = self.try_send(msg) {
            println!("Error enviando mensaje recibido: {}", e);
        }
    }
}
