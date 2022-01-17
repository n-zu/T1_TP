use mqtt_client::{Message, Observer as ObserverTrait};
use std::sync::{mpsc::Sender, Arc, Mutex};

#[derive(Clone)]
pub struct Observer {
    sender: Arc<Mutex<Sender<String>>>,
}

impl ObserverTrait for Observer {
    fn update(&self, msg: Message) {
        match msg {
            Message::Publish(publish) => {
                let payload = publish.payload();
                self.sender
                    .lock()
                    .unwrap()
                    .send(payload.to_string())
                    .expect("Error inesperado: No se pudo enviar mensaje por channel");
                println!("RECEIVED: [{}]", payload);
            }
            _ => println!("[ {:?} ]", msg),
        }
    }
}

impl Observer {
    pub fn new(sender: Sender<String>) -> Self {
        Observer {
            sender: Arc::new(Mutex::new(sender)),
        }
    }
}
