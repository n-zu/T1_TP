use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::Rc;

mod client_observer;
mod publication_counter;
mod subscription_list;
mod utils;

use crate::interface::client_observer::ClientObserver;

use crate::client::{Client, ClientError};

use gtk::gdk::keys::constants::Return;
use gtk::gdk::EventKey;
use gtk::glib::GString;
use gtk::prelude::{ComboBoxTextExt, StackExt, SwitchExt, WidgetExt};
use gtk::{
    prelude::{BuilderExtManual, ButtonExt, EntryExt, TextBufferExt},
    Builder, Button, Entry, Label, Notebook, Switch, TextBuffer,
};
use gtk::{ComboBoxText, Inhibit, ListBox, Stack, TextView, Window};
use packets::connect::{Connect, ConnectBuilder, LastWill};
use packets::topic_filter::TopicFilter;

use crate::interface::publication_counter::PublicationCounter;
use packets::publish::Publish;
use packets::qos::QoSLevel;
use packets::subscribe::Subscribe;
use packets::unsubscribe::Unsubscribe;

use self::subscription_list::SubscriptionList;
use self::utils::{Icon, InterfaceUtils};

/// Controller for the client. It both creates the
/// internal client and handles all the user inputs
/// from the interface
pub struct Controller {
    builder: Builder,
    client: RefCell<Option<Client<ClientObserver>>>,
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
            client: RefCell::new(None),
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
        self.setup_keypress();
    }

    #[doc(hidden)]
    /// Sets up the key press handler
    fn setup_keypress(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let window: Window = self.builder.object("main_window").unwrap();
        window.connect_key_press_event(move |_, event| {
            cont_clone.handle_keypress(event);
            Inhibit(false)
        });
    }

    #[doc(hidden)]
    /// Sets up the connect button
    fn setup_connect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let connect: Button = self.builder.object("con_btn").unwrap();
        connect.connect_clicked(move |button: &Button| {
            cont_clone.handle_connect(button);
        });
    }

    #[doc(hidden)]
    /// Sets up the subscribe button
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
    /// Sets up the disconnect button
    fn setup_disconnect(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let disconnect: Button = self.builder.object("discon_btn").unwrap();
        disconnect.connect_clicked(move |button: &Button| {
            cont_clone.handle_disconnect(button);
        });
    }

    #[doc(hidden)]
    /// Sets up the unsubscribe button
    fn setup_unsubscribe(self: &Rc<Self>) {
        let cont_clone = self.clone();
        let unsubscribe: Button = self.builder.object("unsub_btn").unwrap();
        unsubscribe.connect_clicked(move |button: &Button| {
            cont_clone.handle_unsubscribe(button);
        });
    }

    #[doc(hidden)]
    /// Keypress handler (Connect on enter key press)
    fn handle_keypress(&self, event: &EventKey) {
        let lw: TextView = self.builder.object("con_lw_msg").unwrap();
        let stack: Stack = self.builder.object("content").unwrap();
        let current_tab = stack.visible_child_name();
        if event.keyval() == Return
            && matches!(
                &current_tab.as_ref().map(GString::as_str),
                Some("box_connection")
            )
            && !lw.is_focus()
        {
            self.builder.object::<Button>("con_btn").unwrap().clicked();
        }
    }

    #[doc(hidden)]
    /// Retrieves all the necessary input data from the UI in order to create and connect
    /// a new Client
    fn _connect(&self) -> Result<(), ClientError> {
        let address_entry: Entry = self.builder.object("con_host").unwrap();
        let port_entry: Entry = self.builder.object("con_port").unwrap();
        let con_user_entry: Entry = self.builder.object("con_usr").unwrap();
        let con_client_id_entry: Entry = self.builder.object("con_cli").unwrap();
        let full_addr = format!(
            "{}:{}",
            &address_entry.text().to_string(),
            &port_entry.text().to_string()
        );
        let full_client = format!(
            "Usuario: {} - ID Cliente: {}",
            con_user_entry.text().to_string(),
            con_client_id_entry.text().to_string()
        );

        let connect = self.create_connect_packet()?;
        let client_observer = self.create_client_observer();
        let client = Client::new(&full_addr, client_observer, connect)?;

        self.connection_info(Some(&format!(
            "Conectado a {} ({})",
            full_addr, full_client
        )));
        self.client.borrow_mut().replace(client);

        Ok(())
    }

    #[doc(hidden)]
    /// Builds a ClientObserver
    fn create_client_observer(&self) -> ClientObserver {
        let sub_box: ListBox = self.builder.object("sub_subs").unwrap();
        let unsub_entry: Entry = self.builder.object("unsub_top").unwrap();
        let notebook: Notebook = self.builder.object("notebook").unwrap();
        let feed_label: Label = self.builder.object("label_incoming").unwrap();
        let subs_list = SubscriptionList::new(sub_box, unsub_entry);
        let publication_counter = PublicationCounter::new(notebook, feed_label);
        ClientObserver::new(self.builder.clone(), subs_list, publication_counter)
    }

    #[doc(hidden)]
    /// Creates a new CONNECT packet given the input data from the UI
    fn create_connect_packet(&self) -> Result<Connect, ClientError> {
        // Get the entries from the interface
        let id_entry: Entry = self.builder.object("con_cli").unwrap();
        let user_entry: Entry = self.builder.object("con_usr").unwrap();
        let password_entry: Entry = self.builder.object("con_psw").unwrap();
        let keep_alive_entry: Entry = self.builder.object("con_ka").unwrap();
        let clean_session_switch: Switch = self.builder.object("con_cs").unwrap();
        let last_will_retain_switch: Switch = self.builder.object("con_lw_ret").unwrap();
        let last_will_topic_entry: Entry = self.builder.object("con_lw_top").unwrap();
        let last_will_msg_entry: TextBuffer = self.builder.object("con_lw_txtbuffer").unwrap();
        let last_will_qos_entry: ComboBoxText = self.builder.object("con_lw_qos").unwrap();

        // Get the values from the entries
        let user_name = user_entry.text().to_string();
        let password = password_entry.text().to_string();
        let keep_alive = keep_alive_entry.text().to_string().parse::<u16>()?;
        let client_id = id_entry.text().to_string();
        let clean_session = clean_session_switch.is_active();
        let last_will_retain = last_will_retain_switch.is_active();
        let last_will_topic = last_will_topic_entry.text().to_string();
        let last_will_msg = last_will_msg_entry
            .text(
                &last_will_msg_entry.start_iter(),
                &last_will_msg_entry.end_iter(),
                false,
            )
            .unwrap_or_else(|| GString::from(""))
            .to_string();
        let last_will_qos = last_will_qos_entry
            .active_text()
            .unwrap()
            .to_string()
            .parse::<u8>()
            .unwrap();

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
                TopicFilter::new(last_will_topic, QoSLevel::try_from(last_will_qos)?)?,
                last_will_msg,
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
    #[doc(hidden)]
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
    /// Retrieves all the necessary input data from the UI in order to create and send
    /// a new SUBSCRIBE packet
    fn _subscribe(&self) -> Result<(), ClientError> {
        let topic_entry: Entry = self.builder.object("sub_top").unwrap();
        let qos_entry: ComboBoxText = self.builder.object("sub_qos").unwrap();
        let qos = qos_entry
            .active_text()
            .unwrap()
            .to_string()
            .parse::<u8>()
            .unwrap();

        let topic = TopicFilter::new(&topic_entry.text().to_string(), QoSLevel::try_from(qos)?)?;

        let packet = Subscribe::new(vec![topic], rand::random());

        if let Some(client) = self.client.borrow_mut().as_mut() {
            client.subscribe(packet)?;
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }

        Ok(())
    }

    /// Listener of the Subscribe button
    /// Tries to build a SUBSCRIBE packet
    /// and send it to the server with the
    /// given inputs
    #[doc(hidden)]
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
    /// Retrieves all the necessary input data from the UI in order to create and send
    /// a new PUBLISH packet
    fn _publish(&self) -> Result<(), ClientError> {
        let topic_entry: Entry = self.builder.object("pub_top").unwrap();
        let qos_entry: ComboBoxText = self.builder.object("pub_qos").unwrap();
        let retain_switch: Switch = self.builder.object("pub_ret").unwrap();
        let qos = QoSLevel::try_from(
            qos_entry
                .active_text()
                .unwrap()
                .to_string()
                .parse::<u8>()
                .unwrap(),
        )?;
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

        if let Some(client) = self.client.borrow_mut().as_mut() {
            client.publish(packet)?;
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }

        Ok(())
    }

    /// Listener of the Publish button
    /// Tries to build a PUBLISH packet
    /// and send it to the server with the
    /// given inputs
    #[doc(hidden)]
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
    /// Drops the internal Client
    fn _disconnect(&self) -> Result<(), ClientError> {
        self.client.borrow_mut().take();
        Ok(())
    }

    /// Listener of the Disconnect button
    /// Tries to disconnect the client and
    /// allow the user to connect to another
    /// server
    #[doc(hidden)]
    fn handle_disconnect(&self, _: &Button) {
        if let Err(err) = self._disconnect() {
            self.icon(Icon::Error);
            self.status_message(&format!("Error desconectando: {}", err));
        } else {
            self.reset_ui();
            self.show_connect_menu();
            self.icon(Icon::Ok);
            self.status_message("Desconectado");
            self.connection_info(None);
        }
    }

    #[doc(hidden)]
    /// Retrieves all the necessary input data from the UI in order to create and send
    /// a new SUBSCRIBE packet
    fn _unsubscribe(&self) -> Result<(), ClientError> {
        let topic_entry: Entry = self.builder.object("unsub_top").unwrap();
        let text = vec![TopicFilter::new(
            &topic_entry.text().to_string(),
            QoSLevel::QoSLevel0,
        )?];
        let unsubscribe = Unsubscribe::new(rand::random(), text)?;
        if let Some(client) = self.client.borrow_mut().as_mut() {
            client.unsubscribe(unsubscribe)?;
        } else {
            return Err(ClientError::new("No hay una conexión activa"));
        }
        Ok(())
    }

    /// Listener of the Unsubscribe button
    /// Tries to build an UNSUBSCRIBE packet
    /// and send it to the server with the
    /// given inputs
    #[doc(hidden)]
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

    /// Resets both connection and connected screen to theirs default state
    #[doc(hidden)]
    fn reset_ui(&self) {
        self.reset_connection_screen();
        self.reset_connected_screen();
    }

    /// Resets connection screen to its default state
    #[doc(hidden)]
    fn reset_connection_screen(&self) {
        self.set_text_to_entry_box("con_host", "localhost");
        self.set_text_to_entry_box("con_port", "1883");
        self.set_text_to_entry_box("con_cli", "default_client");
        self.set_text_to_entry_box("con_usr", "");
        self.set_text_to_entry_box("con_psw", "");
        self.set_text_to_entry_box("con_ka", "0");
        self.set_text_to_entry_box("con_lw_top", "");
        self.set_buffer_to_text_buffer("con_lw_txtbuffer", "");
        self.set_state_to_switch_box("con_cs", false);
        self.set_state_to_switch_box("con_lw_ret", false);
    }

    /// Resets connected screen to its default state
    #[doc(hidden)]
    fn reset_connected_screen(&self) {
        self.set_text_to_entry_box("pub_top", "top/sub");
        self.set_state_to_switch_box("pub_ret", false);
        self.set_text_to_entry_box("sub_top", "top/sub");
        self.set_text_to_entry_box("unsub_top", "top/sub");
        self.set_buffer_to_text_buffer("pub_mg_txtbuffer", "");
        self.remove_all_children_from_listbox("sub_subs");
        self.remove_all_children_from_listbox("sub_msgs");
    }
}
