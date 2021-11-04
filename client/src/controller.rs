use std::{rc::Rc, sync::Mutex};

use gtk::{
    prelude::{BuilderExtManual, ButtonExt, EntryExt},
    Builder, Button, Entry,
};

use crate::{client::Client, connect::ConnectBuilder};

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
        let cont_clone = self.clone();
        let connect: Button = self.builder.object("connect_button").unwrap();
        connect.connect_clicked(move |button: &Button| {
            cont_clone.handle_connect(button);
        });
    }

    fn handle_connect(&self, _: &Button) {
        println!("Connect button clicked");
        let addr: Entry = self.builder.object("server_address").unwrap();
        let id: Entry = self.builder.object("client_id").unwrap();
        let mut cliente = Client::new(&addr.text().to_string()).unwrap();
        let connect = ConnectBuilder::new(&id.text().to_string(), 0, true)
            .unwrap()
            .build()
            .unwrap();
        cliente.connect(connect).unwrap();
        self.client.lock().unwrap().replace(cliente);
    }
}
