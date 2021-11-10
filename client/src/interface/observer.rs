use gtk::{
    glib,
    prelude::{ContainerExt, WidgetExt},
    Box, Label, ListBox, ListBoxRow, Orientation,
};
use packets::{packet_reader::QoSLevel, publish::Publish};

use crate::observer::{Message, Observer};

#[derive(Clone)]
pub struct ClientObserver {
    sender: glib::Sender<Message>,
}

impl ClientObserver {
    pub fn new(list: ListBox) -> ClientObserver {
        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        receiver.attach(None, move |message: Message| {
            message_receiver(message, &list);
            glib::Continue(true)
        });

        ClientObserver { sender }
    }
}

fn message_receiver(message: Message, list: &ListBox) {
    match message {
        Message::Publish(publish) => {
            println!("Me llegó el publish {:?}", publish);
            add_publish(publish, list);
        }
        Message::Connected(result) => {
            println!("Conectado: {:?}", result);
        }
        _ => (),
    }
}

fn get_box(topic: &str, payload: &str, qos: QoSLevel) -> Box {
    let outer_box = Box::new(Orientation::Vertical, 5);
    let inner_box = Box::new(Orientation::Horizontal, 5);
    inner_box.add(&Label::new(Some(topic)));
    inner_box.add(&Label::new(Some(&format!("QOS: {}", qos as u8))));
    outer_box.add(&inner_box);
    outer_box.add(&Label::new(Some(payload)));
    outer_box
}

fn add_publish(publish: Publish, list: &ListBox) {
    let row = ListBoxRow::new();
    row.add(&get_box(
        publish.topic_name(),
        publish.payload().unwrap_or(&"".to_string()),
        publish.qos(),
    ));
    list.add(&row);
    list.show_all();
}

impl Observer for ClientObserver {
    fn update(&self, message: Message) {
        // Si falla, se rompe la interfaz pero no podemos hacer mucho
        let _ = self.sender.send(message);
    }
}
