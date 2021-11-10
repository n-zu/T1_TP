use std::{rc::Rc, sync::Mutex};
mod client_observer;
use crate::{
    client::ClientError,
    client_packets::{ConnectBuilder, Subscribe, Topic},
    interface::client_observer::ClientObserver,
};

use gtk::{
    prelude::{
        BuilderExtManual, ButtonExt, DialogExt, EntryExt, LabelExt, StackExt, TextBufferExt,
        WidgetExt,
    },
    Box, Builder, Button, ButtonsType, DialogFlags, Entry, Label, MessageDialog, MessageType,
    Stack, TextBuffer, Window,
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
        let stack: Stack = cont.builder.object("content").unwrap();
        stack.set_visible_child_name("box_connection");
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

    fn setup_subscribe(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let subscribe: Button = self.builder.object("sub_btn").unwrap();
        subscribe.connect_clicked(move |button: &Button| {
            cont_clone.handle_subscribe(button);
        });
    }

    fn setup_publish(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let publish: Button = self.builder.object("pub_btn").unwrap();
        publish.connect_clicked(move |button: &Button| {
            cont_clone.handle_publish(button);
        });
    }

    fn _connect(&self) -> Result<(), ClientError> {
        let addr: Entry = self.builder.object("con_host").unwrap();
        let port: Entry = self.builder.object("con_port").unwrap();
        let id: Entry = self.builder.object("con_cli").unwrap();

        let full_addr = format!("{}:{}", &addr.text().to_string(), &port.text().to_string());

        let connect = ConnectBuilder::new(&id.text().to_string(), 0, true)?.build()?;
        let observer = ClientObserver::new(self.builder.clone());
        let client = Client::new(&full_addr, observer, connect)?;

        let info: Label = self.builder.object("connection_info").unwrap();
        info.set_text(&format!("Dirección del servidor: {}", full_addr));
        self.client.lock()?.replace(client);

        Ok(())
    }

    fn handle_connect(&self, _: &Button) {
        let window: Box = self.builder.object("box_connection").unwrap();
        let status_icon: Stack = self.builder.object("status_icon").unwrap();
        let status_text: Label = self.builder.object("status_label").unwrap();
        status_icon.set_visible_child_name("loading");
        status_text.set_text("Conectando... ");
        window.set_sensitive(false);
        if let Err(e) = self._connect() {
            Self::alert(&format!("No se pudo conectar: {}", e));
            window.set_sensitive(true);
            status_icon.set_visible_child_name("error");
            status_text.set_text("No se pudo conectar");
        }
    }

    fn _subscribe(&self) -> Result<(), ClientError> {
        let topic: Entry = self.builder.object("sub_top").unwrap();
        let qos = QoSLevel::QoSLevel0; // TODO

        let topic = Topic::new(&topic.text().to_string(), qos)?;

        let packet = Subscribe::new(vec![topic], 0);

        if let Some(client) = self.client.lock()?.as_mut() {
            client.subscribe(packet)?;
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }

        Ok(())
    }

    fn handle_subscribe(&self, _: &Button) {
        if let Err(e) = self._subscribe() {
            Self::alert(&format!("No se pudo suscribir: {}", e));
        }
    }

    fn _publish(&self) -> Result<(), ClientError> {
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
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }

        Ok(())
    }

    fn handle_publish(self: &Rc<Self>, _: &Button) {
        if let Err(e) = self._publish() {
            Self::alert(&format!("No se pudo publicar: {}", e));
        }
    }

    pub fn alert(message: &str) {
        let dialog = MessageDialog::new(
            None::<&Window>,
            DialogFlags::MODAL,
            MessageType::Error,
            ButtonsType::Close,
            message,
        );
        dialog.run();
        dialog.emit_close();
    }
}
