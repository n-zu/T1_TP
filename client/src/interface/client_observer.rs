use gtk::{
    glib,
    prelude::{BuilderExtManual, ButtonExt, ContainerExt, WidgetExt},
    Box, Builder, Button, Label, ListBox, ListBoxRow, Orientation,
};
use packets::{packet_reader::QoSLevel, puback::Puback, publish::Publish, suback::Suback};

use crate::{
    client::ClientError,
    client_packets::{Connack, Unsuback},
    observer::{Message, Observer},
};

use super::utils::{alert, Icon, InterfaceUtils};

/// Observer for the internal client. It sends all messages through
/// a channel to the main GTK thread.
#[derive(Clone)]
pub struct ClientObserver {
    sender: glib::Sender<Message>,
}

impl Observer for ClientObserver {
    fn update(&self, message: Message) {
        if let Err(e) = self.sender.send(message) {
            alert(&format!("Error interno: {}", e));
        }
    }
}

impl ClientObserver {
    /// Creates a new ClientObserver with the given Builder
    /// of the interface
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

/// Internal structure for the ClientObserver, which stores
/// the interface's Builder and runs in the main GKT thread
struct InternalObserver {
    builder: Builder,
}

impl InterfaceUtils for InternalObserver {
    fn builder(&self) -> &Builder {
        &self.builder
    }
}

impl InternalObserver {
    /// Creates a new InternalObserver with the given
    /// interface builder
    fn new(builder: Builder) -> Self {
        Self { builder }
    }

    /// Receives a message and updates the interface
    /// accordingly
    fn message_receiver(&self, message: Message) {
        match message {
            Message::Publish(publish) => {
                self.add_publish(publish);
            }
            Message::Connected(result) => {
                self.connected(result);
            }
            Message::Published(result) => {
                self.published(result);
            }
            Message::Subscribed(result) => {
                self.subscribed(result);
            }
            Message::Unsubscribed(result) => {
                self.unsubscribed(result);
            }
            Message::InternalError(error) => {
                alert(&format!(
                    "Error interno: {}\n\nSe recomienda reiniciar el cliente",
                    error
                ));
            }
        }
    }

    /// Re-enables the interface and shows information
    /// about the result of the subscribe operation
    fn subscribed(&self, result: Result<Suback, ClientError>) {
        self.sensitive(true);
        match result {
            Ok(suback) => {
                self.icon(Icon::Ok);
                self.status_message("Suscrito");
                self.add_subscription(suback);
            }
            Err(error) => {
                self.icon(Icon::Error);
                self.status_message(&format!("No se pudo suscribir: {}", error));
            }
        }
    }

    fn add_subscription(&self, _result: Suback) {
        let list: ListBox = self.builder.object("sub_subs").unwrap();

        // for each topic in the suback, add a new row to the list
        for topic in _result.topics() {
            let row = ListBoxRow::new();
            row.add(&get_sub_box(topic.name(), topic.qos()));
            list.add(&row);
        }

        list.show_all();
    }

    /// Re-enables the interface and shows information
    /// about the result of the publish operation
    fn published(&self, result: Result<Option<Puback>, ClientError>) {
        self.sensitive(true);
        if let Err(e) = result {
            self.icon(Icon::Error);
            self.status_message(&format!("No se pudo publicar: {}", e));
        } else {
            self.icon(Icon::Ok);
            self.status_message("Publicado");
        }
    }

    /// Adds a new received publish packet to the feed
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

    /// Re-enables the interface and shows information
    /// about the result of the connect operation. If
    /// it succeed it switches to the connected/content menu
    fn connected(&self, result: Result<Connack, ClientError>) {
        if let Err(e) = result {
            self.connection_info(None);
            self.sensitive(true);
            self.icon(Icon::Error);
            self.status_message(&format!("No se pudo conectar: {}", e));
        } else {
            self.show_content_menu();
            self.icon(Icon::Ok);
            self.status_message("Connected");
        }
    }

    /// Re-enables the interfaces and shows information
    /// about the result of the unsubscribe operation
    fn unsubscribed(&self, result: Result<Unsuback, ClientError>) {
        self.sensitive(true);
        if let Err(e) = result {
            self.icon(Icon::Error);
            self.status_message(&format!("No se pudo desuscribir: {}", e));
        } else {
            self.icon(Icon::Ok);
            self.status_message("Desuscrito");
        }
    }
}

#[doc(hidden)]
fn get_box(topic: &str, payload: &str, qos: QoSLevel) -> Box {
    let outer_box = Box::new(Orientation::Vertical, 5);
    let inner_box = Box::new(Orientation::Horizontal, 5);
    inner_box.add(&Label::new(Some(topic)));
    inner_box.add(&Label::new(Some(&format!("QOS: {}", qos as u8))));
    outer_box.add(&inner_box);
    outer_box.add(&Label::new(Some(payload)));
    outer_box
}

#[doc(hidden)]
fn get_sub_box(topic: &str, qos: QoSLevel) -> Box {
    let outer_box = Box::new(Orientation::Horizontal, 5);
    outer_box.add(&Label::new(Some(&format!("QOS: {}", qos as u8))));
    outer_box.add(&Label::new(Some(topic)));

    // ADD UNSUB BUTTON
    let _topic = topic.to_string().clone();
    let button = Button::with_label("Unsubscribe");
    button.connect_clicked(move |_| {
        // TODO: Unsubscribe
        println!("Unsubscribing from {}", _topic);
    });
    outer_box.add(&button);

    outer_box
}
