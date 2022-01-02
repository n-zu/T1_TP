use mqtt_client::{Message, Observer as ObserverTrait};

#[derive(Clone)]
pub struct Observer;

impl ObserverTrait for Observer {
    fn update(&self, msg: Message) {
        match msg {
            Message::Publish(publish) => {
                let payload = publish.payload();
                println!("RECIEVED: [{}]", payload);
            }
            _ => println!("[ {:?} ]", msg),
        }
    }
}
