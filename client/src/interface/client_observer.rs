use gtk::{
    glib,
    prelude::{BuilderExtManual, ContainerExt, LabelExt, StackExt, WidgetExt},
    Box, Builder, Label, ListBox, ListBoxRow, Orientation, Stack,
};
use packets::{packet_reader::QoSLevel, publish::Publish};

use crate::{
    client::ClientError,
    client_packets::Connack,
    interface::Controller,
    observer::{Message, Observer},
};

#[derive(Clone)]
pub struct ClientObserver {
    sender: glib::Sender<Message>,
}

impl Observer for ClientObserver {
    fn update(&self, message: Message) {
        if let Err(e) = self.sender.send(message) {
            Controller::alert(&format!("Error interno: {}", e));
        }
    }
}

impl ClientObserver {
    pub fn new(builder: Builder) -> ClientObserver {
        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        receiver.attach(None, move |message: Message| {
            message_receiver(message, &builder);
            glib::Continue(true)
        });

        ClientObserver { sender }
    }
}

fn message_receiver(message: Message, builder: &Builder) {
    match message {
        Message::Publish(publish) => {
            add_publish(publish, builder);
        }
        Message::Connected(result) => {
            connected(result, builder);
        }
        Message::InternalError(error) => {
            Controller::alert(&format!(
                "Error interno: {}\n\nSe recomienda reiniciar el cliente",
                error
            ));
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

fn add_publish(publish: Publish, builder: &Builder) {
    let list: ListBox = builder.object("sub_msgs").unwrap();

    let row = ListBoxRow::new();
    row.add(&get_box(
        publish.topic_name(),
        publish.payload().unwrap_or(&"".to_string()),
        publish.qos(),
    ));

    list.add(&row);
    list.show_all();
}

fn connected(result: Result<Connack, ClientError>, builder: &Builder) {
    let status_icon: Stack = builder.object("status_icon").unwrap();
    let status_text: Label = builder.object("status_label").unwrap();

    if let Err(e) = result {
        Controller::alert(&format!("No se pudo conectar: {}", e));
        let connect_window: Box = builder.object("box_connection").unwrap();
        let info: Label = builder.object("connection_info").unwrap();

        connect_window.set_sensitive(true);
        info.set_text("");

        status_icon.set_visible_child_name("error");
        status_text.set_text(&format!("No se pudo conectar: {}", e));
    } else {
        let stack: Stack = builder.object("content").unwrap();
        stack.set_visible_child_name("box_connected");
        status_icon.set_visible_child_name("ok");
        status_text.set_text("Connected");
    }
}
