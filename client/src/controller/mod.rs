use std::{rc::Rc, sync::Mutex};
mod publish_controller;
use crate::{
    client_error::ClientError,
    client_packets::{ConnectBuilder, Subscribe, Topic},
    controller::publish_controller::ClientObserver,
};

use gtk::{
    prelude::{BuilderExtManual, ButtonExt, DialogExt, EntryExt, TextBufferExt},
    Builder, Button, ButtonsType, DialogFlags, Entry, ListBox, MessageDialog, MessageType,
    TextBuffer, Window,
};

use crate::client::Client;

use packets::{packet_reader::QoSLevel, publish::Publish};

pub struct Controller {
    builder: Builder,
    client: Mutex<Option<Client<ClientObserver>>>,
}

impl Controller {
    pub fn new(builder: Builder) -> Rc<Self> {
        let cont = Rc::new(Self {
            builder,
            client: Mutex::new(None),
        });
        cont.setup_handlers();
        cont
    }

    fn setup_handlers(self: &Rc<Self>) {
        self.setup_connect();
        self.setup_subscribe();
        self.setup_publish();
    }

    fn setup_connect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let connect: Button = self.builder.object("con_btn").unwrap();
        connect.connect_clicked(move |button: &Button| {
            cont_clone.handle_connect(button);
        });
    }

    fn try_do<F>(&self, function: F)
    where
        F: FnOnce() -> Result<(), ClientError>,
    {
        if let Err(err) = function() {
            self.alert(&format!("No se pudo ejecutar tarea: {}", err));
        }
    }

    fn handle_connect(&self, _: &Button) {
        println!("Connect button clicked");
        self.try_do(|| -> Result<(), ClientError> {
            let addr: Entry = self.builder.object("con_host").unwrap();
            let port: Entry = self.builder.object("con_port").unwrap();
            let id: Entry = self.builder.object("con_cli").unwrap();
            let pub_list: ListBox = self.builder.object("sub_msgs").unwrap();

            let full_addr = format!("{}:{}", &addr.text().to_string(), &port.text().to_string());

            let mut new_client = Client::new(&full_addr, ClientObserver::new(pub_list))?;

            let connect = ConnectBuilder::new(&id.text().to_string(), 0, true)?.build()?;

            match new_client.connect(connect) {
                Result::Ok(()) => {
                    println!("Connected to server");
                    self.client.lock()?.replace(new_client);
                }
                Err(e) => {
                    println!("Failed to connect to server: {}", e);
                    self.client.lock()?.take();
                }
            }

            Ok(())
        });
    }

    fn setup_subscribe(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let subscribe: Button = self.builder.object("sub_btn").unwrap();
        subscribe.connect_clicked(move |button: &Button| {
            cont_clone.handle_subscribe(button);
        });
    }

    fn handle_subscribe(&self, _: &Button) {
        println!("Subscribe button clicked");

        self.try_do(|| -> Result<(), ClientError> {
            let topic: Entry = self.builder.object("sub_top").unwrap();
            let qos = QoSLevel::QoSLevel0; // TODO

            let topic = Topic::new(&topic.text().to_string(), qos)?;

            let packet = Subscribe::new(vec![topic], 0);

            if let Some(client) = self.client.lock()?.as_mut() {
                client.subscribe(packet)?;
            }

            Ok(())
        });
    }

    fn setup_publish(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let publish: Button = self.builder.object("pub_btn").unwrap();
        publish.connect_clicked(move |button: &Button| {
            cont_clone.handle_publish(button);
        });
    }

    fn handle_publish(self: &Rc<Self>, _: &Button) {
        println!("Publish button clicked");

        self.try_do(|| -> Result<(), ClientError> {
            let topic: Entry = self.builder.object("pub_top").unwrap();
            let qos = QoSLevel::QoSLevel0; // TODO
            let retain = false; // TODO
            let msg: TextBuffer = self.builder.object("pub_mg_txtbuffer").unwrap();

            let packet = Publish::new(
                false, // TODO
                qos,
                retain,
                &topic.text().to_string(),
                &msg.text(&msg.start_iter(), &msg.end_iter(), false)
                    .ok_or_else(|| ClientError::new("Se debe completar el campo de mensaje"))?,
                None, // TODO
            )?;

            if let Some(client) = self.client.lock()?.as_mut() {
                client.publish(packet)?;
            }

            Ok(())
        });
    }

    fn alert(&self, message: &str) {
        MessageDialog::new(
            None::<&Window>,
            DialogFlags::empty(),
            MessageType::Error,
            ButtonsType::Ok,
            message,
        )
        .run();
    }
}
