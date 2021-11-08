use gtk::{
    glib,
    prelude::{ContainerExt, WidgetExt},
    Box, Label, ListBox, ListBoxRow, Orientation,
};
use packets::{packet_reader::QoSLevel, publish::Publish};

use super::super::client::PublishObserver;

#[derive(Clone)]
pub struct PublishController {
    sender: glib::Sender<Publish>,
}

impl PublishController {
    pub fn new(list: ListBox) -> PublishController {
        let function = move |publish: Publish| {
            println!("Me llegó el publish {:?}", publish);
            add_publish(publish, &list);
        };

        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        receiver.attach(None, move |publish: Publish| {
            function(publish);
            glib::Continue(true)
        });

        PublishController { sender }
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

impl PublishObserver for PublishController {
    fn got_publish(&self, publish: Publish) {
        self.sender.send(publish).unwrap();
    }
}
