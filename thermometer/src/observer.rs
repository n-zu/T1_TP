use mqtt_client::{Message, Observer};

#[derive(Clone)]
pub struct MyObserver;

impl Observer for MyObserver {
    fn update(&self, msg: Message) {
        println!("{:?}", msg);
    }
}
