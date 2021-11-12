use std::{rc::Rc, sync::Mutex};
mod client_observer;
mod utils;
use crate::{
    client::ClientError,
    client_packets::{ConnectBuilder, Subscribe, Topic},
    interface::client_observer::ClientObserver,
};

use crate::client::Client;

use gtk::{
    prelude::{BuilderExtManual, ButtonExt, EntryExt, TextBufferExt},
    Builder, Button, Entry, TextBuffer,
};

use crate::client_packets::Connect;
use packets::{packet_reader::QoSLevel, publish::Publish};

use self::utils::{Icon, InterfaceUtils};

pub struct Controller {
    builder: Builder,
    client: Mutex<Option<Client<ClientObserver>>>,
}

impl InterfaceUtils for Controller {
    fn builder(&self) -> &Builder {
        &self.builder
    }
}

impl Controller {
    pub fn new(builder: Builder) -> Rc<Self> {
        let cont = Rc::new(Self {
            builder,
            client: Mutex::new(None),
        });
        cont.setup_handlers();
        cont.show_connect_menu();
        cont
    }

    fn setup_handlers(self: &Rc<Self>) {
        self.setup_connect();
        self.setup_subscribe();
        self.setup_publish();
        self.setup_disconnect();
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

    fn setup_disconnect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let disconnect: Button = self.builder.object("discon_btn").unwrap();
        disconnect.connect_clicked(move |button: &Button| {
            cont_clone.handle_disconnect(button);
        });
    }

    fn _connect(&self) -> Result<(), ClientError> {
        let address_entry: Entry = self.builder.object("con_host").unwrap();
        let port_entry: Entry = self.builder.object("con_port").unwrap();
        let id_entry: Entry = self.builder.object("con_cli").unwrap();
        let user_entry: Entry = self.builder.object("con_usr").unwrap();
        let password_entry: Entry = self.builder.object("con_psw").unwrap();
        let keep_alive_entry: Entry = self.builder.object("con_ka").unwrap();
        let _clean_session_entry: Entry = self.builder.object("con_cs").unwrap();

        let full_addr = format!(
            "{}:{}",
            &address_entry.text().to_string(),
            &port_entry.text().to_string()
        );
        let user_name = user_entry.text().to_string();
        let password = password_entry.text().to_string();
        let keep_alive: u16 = keep_alive_entry.text().to_string().parse().unwrap();
        let client_id = id_entry.text().to_string();

        let connect =
            Self::_create_connect_packet(&client_id, &user_name, &password, keep_alive, true)?;
        let observer = ClientObserver::new(self.builder.clone());
        let client = Client::new(&full_addr, observer, connect)?;

        self.connection_info(Some(&format!("Conectado a {}", full_addr)));
        self.client.lock()?.replace(client);

        Ok(())
    }

    fn _create_connect_packet(
        client_id: &str,
        user_name: &str,
        password: &str,
        keep_alive: u16,
        clean_session: bool,
    ) -> Result<Connect, ClientError> {
        let mut connect_builder = ConnectBuilder::new(client_id, keep_alive, clean_session)?;
        if !user_name.is_empty() {
            connect_builder = connect_builder.user_name(user_name)?;
        }
        if !password.is_empty() {
            connect_builder = connect_builder.password(password)?;
        }
        Ok(connect_builder.build()?)
    }

    fn handle_connect(&self, _: &Button) {
        self.icon(Icon::Loading);
        self.status_message("Conectando...");
        self.sensitive(false);
        if let Err(e) = self._connect() {
            self.sensitive(true);
            self.icon(Icon::Error);
            self.status_message(&format!("No se pudo conectar ({})", e));
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
        self.sensitive(false);
        self.status_message("Suscribiendose...");
        self.icon(Icon::Loading);
        if let Err(e) = self._subscribe() {
            self.sensitive(true);
            self.status_message(&format!("No se pudo suscribir: {}", e));
            self.icon(Icon::Error);
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
        self.sensitive(false);
        self.status_message("Publicando...");
        self.icon(Icon::Loading);
        if let Err(err) = self._publish() {
            self.sensitive(true);
            self.status_message(&format!("No se pudo publicar: {}", err));
            self.icon(Icon::Error);
        }
    }

    fn _disconnect(&self) -> Result<(), ClientError> {
        self.client.lock()?.take();
        Ok(())
    }

    fn handle_disconnect(self: &Rc<Self>, _: &Button) {
        if let Err(err) = self._disconnect() {
            self.icon(Icon::Error);
            self.status_message(&format!("Error desconectando: {}", err));
        } else {
            self.show_connect_menu();
            self.icon(Icon::Ok);
            self.status_message("Desconectado");
            self.connection_info(None);
        }
    }
}
