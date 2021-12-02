mod login;

use std::{collections::HashMap, io::Write, sync::Mutex};

use packets::{connack::ConnackReturnCode, connect::Connect, publish::Publish};
use tracing::debug;

use crate::{
    client::Client,
    network_connection::NetworkConnection,
    server::{server_error::ServerErrorKind, ClientId, ClientIdArg, ServerError, ServerResult},
    traits::{BidirectionalStream, Id},
};

const GENERIC_ID_SUFFIX: &str = "__CLIENT__";

pub struct ClientsManager<S, I>
where
    S: BidirectionalStream,
    I: Id,
{
    clients: HashMap<ClientId, Mutex<Client<S, I>>>,
    accounts_path: String,
    generic_ids_counter: u32,
}

pub struct DisconnectInfo {
    pub publish_last_will: Option<Publish>,
    pub clean_session: bool,
}

pub struct ConnectInfo {
    pub id: ClientId,
    pub session_present: bool,
    pub return_code: ConnackReturnCode,
    pub takeover_last_will: Option<Publish>,
}

impl<S, I> ClientsManager<S, I>
where
    S: BidirectionalStream,
    I: Id + Clone + Copy + std::hash::Hash + Eq,
{
    pub fn new(accounts_path: &str) -> Self {
        Self {
            clients: HashMap::new(),
            accounts_path: accounts_path.to_owned(),
            generic_ids_counter: 0,
        }
    }

    pub fn client_do<F>(&self, id: &ClientIdArg, action: F) -> ServerResult<()>
    where
        F: FnOnce(std::sync::MutexGuard<'_, Client<S, I>>) -> ServerResult<()>,
        S: Write,
    {
        match self.clients.get(id) {
            Some(session) => {
                action(session.lock()?)?;
                Ok(())
            }
            None => Err(ServerError::new_kind(
                &format!("No existe el cliente con id <{}>", id),
                ServerErrorKind::ClientNotFound,
            )),
        }
    }

    pub fn get_client_property<F, T>(&self, id: &ClientIdArg, action: F) -> ServerResult<T>
    where
        F: FnOnce(std::sync::MutexGuard<'_, Client<S, I>>) -> ServerResult<T>,
    {
        match self.clients.get(id) {
            Some(session) => action(session.lock()?),
            None => Err(ServerError::new_kind(
                &format!("No existe el cliente con id <{}>", id),
                ServerErrorKind::ClientNotFound,
            )),
        }
    }

    pub fn disconnect(
        &mut self,
        id: &ClientIdArg,
        connection_stream: NetworkConnection<S, I>,
        gracefully: bool,
    ) -> ServerResult<DisconnectInfo> {
        // Chequeo si ya fue desconectado por el proceso
        // de Client Take-Over
        let old_addr = self.get_client_property(id, |client| Ok(client.connection_id()))?;
        if let Some(old_addr) = old_addr {
            if connection_stream.id() != old_addr {
                return Ok(DisconnectInfo {
                    publish_last_will: None,
                    clean_session: false,
                });
            }
        }

        let publish_last_will =
            self.get_client_property(id, |mut session| session.disconnect(gracefully))?;
        let clean_session;
        // Si la funcion anterior no devolvio error, entonces existe el cliente
        if self
            .clients
            .get(id)
            .expect("Error inesperado: no se encontro el cliente en el HashMap")
            .lock()?
            .clean_session()
        {
            debug!("<{}>: Terminando sesion con clean_session = true", id);
            self.clients.remove(id);
            clean_session = true;
        } else {
            debug!("<{}>: Terminando sesion con clean_session = false", id);
            clean_session = false;
        }
        Ok(DisconnectInfo {
            publish_last_will,
            clean_session,
        })
    }

    fn client_add(&mut self, session: Client<S, I>) -> Option<Mutex<Client<S, I>>> {
        self.clients
            .insert(session.id().to_owned(), Mutex::new(session))
    }

    pub fn send_unacknowledged(&self, id: &ClientIdArg) -> ServerResult<()> {
        self.client_do(id, |mut session| {
            session.send_unacknowledged()?;
            Ok(())
        })
    }

    pub fn send_publish(&self, id: &ClientIdArg, publish: Publish) -> ServerResult<()> {
        self.client_do(id, |mut session| {
            session.send_publish(publish)?;
            Ok(())
        })
    }

    fn check_taken_ids(&mut self, id: &ClientIdArg, user_name: &str) -> ServerResult<()> {
        if let Some(client) = self.clients.get(id) {
            if user_name != client.lock()?.user_name().unwrap() {
                return Err(ServerError::new_kind(
                    &format!("La ID <{}> se encuentra reservada por otro usuario", id),
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
                ));
            } else {
                return Ok(());
            }
        }
        Ok(())
    }

    fn check_credentials(&mut self, connect: &Connect) -> ServerResult<()> {
        if !connect.client_id().chars().all(char::is_alphanumeric) {
            return Err(ServerError::new_kind(
                "Las IDs solo pueden contener caracteres alfanumericos",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
            ));
        }

        if let Some(user_name) = connect.user_name() {
            let password = login::search_password(&self.accounts_path, user_name)?;
            if password == *connect.password().unwrap_or(&String::new()) {
                self.check_taken_ids(connect.client_id(), user_name)
            } else {
                Err(ServerError::new_kind(
                    "Contraseña incorrecta",
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::BadUserNameOrPassword),
                ))
            }
        } else {
            Err(ServerError::new_kind(
                "Clientes sin user_name no estan permitidos",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::NotAuthorized),
            ))
        }
    }

    fn new_generic_id(&mut self) -> String {
        self.generic_ids_counter += 1;
        let mut generic_id = String::from(GENERIC_ID_SUFFIX);
        generic_id.push_str(&self.generic_ids_counter.to_string());
        generic_id
    }

    fn process_client_empty_id(&mut self, connect: &mut Connect) -> ServerResult<()> {
        if !connect.clean_session() {
            Err(ServerError::new_kind(
                "Clientes con id vacia deben tener clean connection_stream en true",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
            ))
        } else {
            let id = self.new_generic_id();
            connect.set_id(id);
            Ok(())
        }
    }

    fn new_session_unchecked(
        &mut self,
        network_connection: &NetworkConnection<S, I>,
        mut connect: Connect,
    ) -> ServerResult<ConnectInfo> {
        if connect.client_id().is_empty() {
            self.process_client_empty_id(&mut connect)?;
        }
        let id = connect.client_id().to_owned();

        let mut takeover_last_will = None;
        let session_present;
        let return_code;

        // Hay una sesion_presente en el servidor con la misma ID
        if let Some(old_client) = self.clients.get_mut(&id) {
            takeover_last_will = old_client
                .get_mut()?
                .reconnect(connect, network_connection.try_clone()?)?;
            session_present = true;
        } else {
            let client = Client::new(connect, network_connection.try_clone()?);
            self.client_add(client);
            session_present = false;
        }
        return_code = ConnackReturnCode::Accepted;
        Ok(ConnectInfo {
            id,
            session_present,
            return_code,
            takeover_last_will,
        })
    }

    pub fn new_session(
        &mut self,
        connection_stream: &NetworkConnection<S, I>,
        connect: Connect,
    ) -> ServerResult<ConnectInfo> {
        self.check_credentials(&connect)?;

        self.new_session_unchecked(connection_stream, connect)
    }

    pub fn finish_all_sessions(&mut self, gracefully: bool) -> ServerResult<()> {
        for (id, session) in &self.clients {
            debug!("<{}>: Desconectando", id);
            session.lock()?.disconnect(gracefully)?;
        }
        self.clients.retain(|_id, session| match session.get_mut() {
            Ok(session) => session.clean_session(),
            Err(_) => false,
        });
        Ok(())
    }
}
