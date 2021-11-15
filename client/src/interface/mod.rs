use std::convert::TryFrom;
use std::{rc::Rc, sync::Mutex};

mod client_observer;
mod utils;
use crate::{
    client::ClientError,
    client_packets::{ConnectBuilder, Subscribe},
    interface::client_observer::ClientObserver,
};

use crate::client::Client;

use gtk::prelude::SwitchExt;
use gtk::{
    prelude::{BuilderExtManual, ButtonExt, EntryExt, TextBufferExt},
    Builder, Button, Entry, Switch, TextBuffer,
};
use packets::topic::Topic;
use packets::utf8::Field;

use crate::client_packets::{Connect, LastWill};
use packets::publish::Publish;
use packets::qos::QoSLevel;

use self::utils::{Icon, InterfaceUtils};

/// Controller for the client. It both creates the
/// internal client and handles all the user inputs
/// from the interface
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
    /// Creates a new Controller with the given
    /// interface builder
    pub fn new(builder: Builder) -> Rc<Self> {
        let cont = Rc::new(Self {
            builder,
            client: Mutex::new(None),
        });
        cont.setup_handlers();
        cont.show_connect_menu();
        cont
    }

    /// Sets up the different listeners for
    /// the interface buttons
    fn setup_handlers(self: &Rc<Self>) {
        self.setup_connect();
        self.setup_subscribe();
        self.setup_publish();
        self.setup_disconnect();
        self.setup_unsubscribe();
    }

    #[doc(hidden)]
    fn setup_connect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let connect: Button = self.builder.object("con_btn").unwrap();
        connect.connect_clicked(move |button: &Button| {
            cont_clone.handle_connect(button);
        });
    }

    #[doc(hidden)]
    fn setup_subscribe(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let subscribe: Button = self.builder.object("sub_btn").unwrap();
        subscribe.connect_clicked(move |button: &Button| {
            cont_clone.handle_subscribe(button);
        });
    }

    #[doc(hidden)]
    fn setup_publish(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let publish: Button = self.builder.object("pub_btn").unwrap();
        publish.connect_clicked(move |button: &Button| {
            cont_clone.handle_publish(button);
        });
    }

    #[doc(hidden)]
    fn setup_disconnect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let disconnect: Button = self.builder.object("discon_btn").unwrap();
        disconnect.connect_clicked(move |button: &Button| {
            cont_clone.handle_disconnect(button);
        });
    }

    #[doc(hidden)]
    fn setup_unsubscribe(self: &Rc<Self>) {
        let _cont_clone = self.clone();
        /*let unsubscribe: Button = self.builder.object("unsub_btn").unwrap();
        unsubscribe.connect_clicked(move |button: &Button| {
            cont_clone.handle_unsubscribe(button);
        });*/ //TODO
    }

    #[doc(hidden)]
    fn _connect(&self) -> Result<(), ClientError> {
        let address_entry: Entry = self.builder.object("con_host").unwrap();
        let port_entry: Entry = self.builder.object("con_port").unwrap();
        let full_addr = format!(
            "{}:{}",
            &address_entry.text().to_string(),
            &port_entry.text().to_string()
        );

        let connect = self._create_connect_packet()?;
        let observer = ClientObserver::new(self.builder.clone());
        let client = Client::new(&full_addr, observer, connect)?;

        self.connection_info(Some(&format!("Conectado a {}", full_addr)));
        self.client.lock()?.replace(client);

        Ok(())
    }

    #[doc(hidden)]
    fn _create_connect_packet(&self) -> Result<Connect, ClientError> {
        // Get the entries from the interface
        let id_entry: Entry = self.builder.object("con_cli").unwrap();
        let user_entry: Entry = self.builder.object("con_usr").unwrap();
        let password_entry: Entry = self.builder.object("con_psw").unwrap();
        let keep_alive_entry: Entry = self.builder.object("con_ka").unwrap();
        let clean_session_switch: Switch = self.builder.object("con_cs").unwrap();
        let last_will_retain_switch: Switch = self.builder.object("con_lw_ret").unwrap();
        let last_will_topic_entry: Entry = self.builder.object("con_lw_top").unwrap();
        let last_will_msg_entry: Entry = self.builder.object("con_lw_msg").unwrap();
        let last_will_qos_entry: Entry = self.builder.object("con_lw_qos_entry").unwrap();

        // Get the values from the entries
        let user_name = user_entry.text().to_string();
        let password = password_entry.text().to_string();
        let keep_alive = keep_alive_entry
            .text()
            .to_string()
            .parse::<u16>()
            .unwrap_or(0);
        let client_id = id_entry.text().to_string();
        let clean_session = clean_session_switch.is_active();
        let last_will_retain = last_will_retain_switch.is_active();
        let last_will_topic = last_will_topic_entry.text().to_string();
        let last_will_msg = last_will_msg_entry.text().to_string();
        let last_will_qos = last_will_qos_entry
            .text()
            .to_string()
            .parse::<u8>()
            .unwrap_or(0);

        // Create the connect builder
        let mut connect_builder = ConnectBuilder::new(&client_id, keep_alive, clean_session)?;
        if !user_name.is_empty() {
            connect_builder = connect_builder.user_name(&user_name)?;
        }
        if !password.is_empty() {
            connect_builder = connect_builder.password(&password)?;
        }

        if !last_will_topic.trim().is_empty() {
            let last_will = LastWill::new(
                Field::new_from_string(&last_will_topic)?,
                Field::new_from_string(&last_will_msg)?,
                QoSLevel::try_from(last_will_qos)?,
                last_will_retain,
            );
            connect_builder = connect_builder.last_will(last_will);
        }

        // Return the built connect packet
        Ok(connect_builder.build()?)
    }

    /// Listener of the Connect button
    /// Tries to connect the client to the
    /// server with the given inputs, and sets
    /// up the ClientObserver
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

    #[doc(hidden)]
    fn _subscribe(&self) -> Result<(), ClientError> {
        let topic_entry: Entry = self.builder.object("sub_top").unwrap();
        let qos_entry: Entry = self.builder.object("sub_qos_entry").unwrap();
        let qos = qos_entry.text().to_string().parse::<u8>().unwrap_or(0);

        let topic = Topic::new(&topic_entry.text().to_string(), QoSLevel::try_from(qos)?)?;

        let packet = Subscribe::new(vec![topic], 0);

        if let Some(client) = self.client.lock()?.as_mut() {
            client.subscribe(packet)?;
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }

        Ok(())
    }

    /// Listener of the Subscribe button
    /// Tries to build a subscribe packet
    /// and send it to the server with the
    /// given inputs
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

    #[doc(hidden)]
    fn _publish(&self) -> Result<(), ClientError> {
        let topic_entry: Entry = self.builder.object("pub_top").unwrap();
        let qos_entry: Entry = self.builder.object("pub_qos_entry").unwrap();
        let retain_switch: Switch = self.builder.object("pub_ret").unwrap();
        let qos = QoSLevel::try_from(qos_entry.text().to_string().parse::<u8>().unwrap_or(0))?;
        let mut id = None;
        if qos != QoSLevel::QoSLevel0 {
            id = Some(rand::random());
        }

        let retain = retain_switch.is_active();
        let msg: TextBuffer = self.builder.object("pub_mg_txtbuffer").unwrap();

        let packet = Publish::new(
            false,
            qos,
            retain,
            &topic_entry.text().to_string(),
            &msg.text(&msg.start_iter(), &msg.end_iter(), false)
                .ok_or_else(|| ClientError::new("Se debe completar el campo de mensaje"))?,
            id,
        )?;

        if let Some(client) = self.client.lock()?.as_mut() {
            client.publish(packet)?;
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }

        Ok(())
    }

    /// Listener of the Publish button
    /// Tries to build a publish packet
    /// and send it to the server with the
    /// given inputs
    fn handle_publish(&self, _: &Button) {
        self.sensitive(false);
        self.status_message("Publicando...");
        self.icon(Icon::Loading);
        if let Err(err) = self._publish() {
            self.sensitive(true);
            self.status_message(&format!("No se pudo publicar: {}", err));
            self.icon(Icon::Error);
        }
    }

    #[doc(hidden)]
    fn _disconnect(&self) -> Result<(), ClientError> {
        self.client.lock()?.take();
        Ok(())
    }

    /// Listener of the Disconnect button
    /// Tries to disconnect the client and
    /// allow the user to connect to another
    /// server
    fn handle_disconnect(&self, _: &Button) {
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

    #[doc(hidden)]
    fn _unsubscribe(&self) -> Result<(), ClientError> {
        // TODO
        Err(ClientError::new("No implementado"))
    }

    /// Listener of the Unsubscribe button
    /// Tries to build an unsubscribe packet
    /// and send it to the server with the
    /// given inputs
    #[allow(dead_code)]
    fn handle_unsubscribe(&self, _: &Button) {
        self.sensitive(false);
        self.status_message("Desuscribiendose...");
        self.icon(Icon::Loading);
        if let Err(e) = self._unsubscribe() {
            self.sensitive(true);
            self.status_message(&format!("No se pudo desuscribir: {}", e));
            self.icon(Icon::Error);
        }
    }
}
