use gtk::{
    glib,
    prelude::{BuilderExtManual, ContainerExt, WidgetExt},
    Box, Builder, Label, ListBox, ListBoxRow, Orientation,
};
use packets::{packet_reader::QoSLevel, publish::Publish};

use crate::{
    client::ClientError,
    client_packets::Connack,
    interface::Controller,
    observer::{Message, Observer},
};

use super::utils::{Icon, InterfaceUtils};

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
        let internal = InternalObserver::new(builder);
        receiver.attach(None, move |message: Message| {
            internal.message_receiver(message);
            glib::Continue(true)
        });

        ClientObserver { sender }
    }
}

struct InternalObserver {
    builder: Builder,
}

impl InterfaceUtils for InternalObserver {
    fn builder(&self) -> &Builder {
        &self.builder
    }
}

impl InternalObserver {
    fn new(builder: Builder) -> Self {
        Self { builder }
    }

    fn message_receiver(&self, message: Message) {
        match message {
            Message::Publish(publish) => {
                self.add_publish(publish);
            }
            Message::Connected(result) => {
                self.connected(result);
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

    fn add_publish(&self, publish: Publish) {
        let list: ListBox = self.builder.object("sub_msgs").unwrap();

        let row = ListBoxRow::new();
        row.add(&get_box(
            publish.topic_name(),
            publish.payload().unwrap_or(&"".to_string()),
            publish.qos(),
        ));

        list.add(&row);
        list.show_all();
    }

    fn connected(&self, result: Result<Connack, ClientError>) {
        if let Err(e) = result {
            self.connection_info("");
            self.sensitive_connect_menu(true);
            self.icon(Icon::Error);
            self.status_message(&format!("No se pudo conectar: {}", e));
        } else {
            self.show_content_menu();
            self.icon(Icon::Ok);
            self.status_message("Connected");
        }
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
