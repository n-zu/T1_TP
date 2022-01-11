use mqtt_client::{Message, Observer};

#[derive(Clone)]
pub struct ThermometerObserver;

impl Observer for ThermometerObserver {
    fn update(&self, msg: Message) {
        println!("[ {:?} ]", msg);
    }
}
