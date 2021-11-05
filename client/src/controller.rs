use std::{rc::Rc, sync::Mutex};

use gtk::{
    prelude::{BuilderExtManual, ButtonExt, EntryExt},
    Builder, Button, Entry,
};

use crate::{
    client::Client,
    connect::{ConnectBuilder, QoSLevel},
    subscribe::{Subscribe, Topic},
};

pub struct Controller {
    builder: Builder,
    client: Mutex<Option<Client>>,
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
    }

    fn setup_connect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let connect: Button = self.builder.object("connect_button").unwrap();
        connect.connect_clicked(move |button: &Button| {
            cont_clone.handle_connect(button);
        });
    }

    fn handle_connect(&self, _: &Button) {
        println!("Connect button clicked");
        let addr: Entry = self.builder.object("server_address").unwrap();
        let port: Entry = self.builder.object("server_port").unwrap();
        let id: Entry = self.builder.object("client_id").unwrap();
        let full_addr = format!("{}:{}", &addr.text().to_string(), &port.text().to_string());

        let mut new_client = Client::new(&full_addr).unwrap();
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
        let subscribe: Button = self.builder.object("subscribe_button").unwrap();
        subscribe.connect_clicked(move |button: &Button| {
            cont_clone.handle_subscribe(button);
        });
    }

    fn handle_subscribe(&self, _: &Button) {
        println!("Subscribe button clicked");
        let topic: Entry = self.builder.object("sub_topic").unwrap();
        let qos = QoSLevel::QoSLevel0; // TODO

        let topic = Topic::new(&topic.text().to_string(), qos).unwrap();

        let packet = Subscribe::new(vec![topic], 0);

        if let Some(client) = self.client.lock().unwrap().as_mut() {
            client.subscribe(packet).unwrap();
        }
    }
}
