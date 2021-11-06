use std::{rc::Rc, sync::Mutex};
mod publish_controller;
use publish_controller::PublishController;

use gtk::{
    prelude::{BuilderExtManual, ButtonExt, EntryExt, ListBoxExt, TextBufferExt},
    Builder, Button, Entry, Label, ListBox, TextBuffer,
};

use crate::{
    client::Client,
    connect::{ConnectBuilder, QoSLevel},
    publish::Publish,
    subscribe::{Subscribe, Topic},
};

pub struct Controller {
    builder: Builder,
    client: Mutex<Option<Client<PublishController>>>,
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

    fn handle_connect(&self, _: &Button) {
        println!("Connect button clicked");
        let addr: Entry = self.builder.object("con_host").unwrap();
        let port: Entry = self.builder.object("con_port").unwrap();
        let id: Entry = self.builder.object("con_cli").unwrap();
        let full_addr = format!("{}:{}", &addr.text().to_string(), &port.text().to_string());

        // FIXME: Crashes if no server is connected
        let pub_list : ListBox= self.builder.object("sub_msgs").unwrap();
        let mut new_client = Client::new(&full_addr, PublishController::new(pub_list)).unwrap();
        let connect = ConnectBuilder::new(&id.text().to_string(), 0, true)
            .unwrap()
            .build()
            .unwrap();
        new_client.connect(connect).unwrap();
        new_client.start_listening();
        self.client.lock().unwrap().replace(new_client);
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
        let topic: Entry = self.builder.object("sub_top").unwrap();
        let qos = QoSLevel::QoSLevel0; // TODO

        let topic = Topic::new(&topic.text().to_string(), qos).unwrap();

        let packet = Subscribe::new(vec![topic], 0);

        if let Some(client) = self.client.lock().unwrap().as_mut() {
            client.subscribe(packet).unwrap();
        }

        // Test
        let mesages: ListBox = self.builder.object("sub_msgs").unwrap();
        mesages.insert(&Label::new(Some("MENSAGE")), 0);
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
        let topic: Entry = self.builder.object("pub_top").unwrap();
        let qos = QoSLevel::QoSLevel0; // TODO
        let retain = false; // TODO
        let msg: TextBuffer = self.builder.object("pub_mg_txtbuffer").unwrap();

        let packet = Publish::new(
            false, // TODO
            qos,
            retain,
            &topic.text().to_string(),
            &msg.text(&msg.start_iter(), &msg.end_iter(), false).unwrap(),
            None, // TODO
        )
        .unwrap();

        if let Some(client) = self.client.lock().unwrap().as_mut() {
            client.publish(packet).unwrap();
        }
    }
}