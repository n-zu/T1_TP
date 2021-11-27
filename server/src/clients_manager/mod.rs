mod login;

use std::{collections::HashMap, net::SocketAddr, sync::Mutex};

use packets::{connack::ConnackReturnCode, connect::Connect, publish::Publish};
use tracing::{debug, warn};

use crate::{
    client::{Client, Session},
    server::{server_error::ServerErrorKind, ClientId, ClientIdArg, ServerError, ServerResult},
};

type Username = String;
const GENERIC_ID_SUFFIX: &str = "__CLIENT__";

pub struct ClientsManager {
    clients: HashMap<ClientId, Mutex<Client>>,
    /// client_id - username
    taken_ids: HashMap<ClientId, Username>,
    current_addrs: HashMap<ClientId, SocketAddr>,
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

impl ClientsManager {
    pub fn new(accounts_path: &str) -> Self {
        Self {
            clients: HashMap::new(),
            taken_ids: HashMap::new(),
            current_addrs: HashMap::new(),
            accounts_path: accounts_path.to_owned(),
            generic_ids_counter: 0,
        }
    }

    pub fn client_do<F>(&self, id: &ClientIdArg, action: F) -> ServerResult<()>
    where
        F: FnOnce(std::sync::MutexGuard<'_, Client>) -> ServerResult<()>,
    {
        match self.clients.get(id) {
            Some(client) => {
                action(client.lock()?)?;
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
        F: FnOnce(std::sync::MutexGuard<'_, Client>) -> ServerResult<T>,
    {
        match self.clients.get(id) {
            Some(client) => action(client.lock()?),
            None => Err(ServerError::new_kind(
                &format!("No existe el cliente con id <{}>", id),
                ServerErrorKind::ClientNotFound,
            )),
        }
    }

    pub fn finish_session(
        &mut self,
        id: &ClientIdArg,
        session: Session,
        gracefully: bool,
    ) -> ServerResult<DisconnectInfo> {
        // Chequeo si ya fue desconectado por el proceso
        // de Client Take-Over
        if let Some(old_addr) = self.current_addrs.get(id) {
            if session.addr() != *old_addr {
                return Ok(DisconnectInfo {
                    publish_last_will: None,
                    clean_session: false,
                });
            }
        }

        let publish_last_will =
            self.get_client_property(id, |mut client| client.disconnect(gracefully))?;
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

    fn client_add(&mut self, client: Client) -> Option<Mutex<Client>> {
        self.clients
            .insert(client.id().to_owned(), Mutex::new(client))
    }

    pub fn send_unacknowledged(&self, id: &ClientIdArg) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.send_unacknowledged()?;
            Ok(())
        })
    }

    pub fn send_publish(&self, id: &ClientIdArg, publish: Publish) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.send_publish(publish)?;
            Ok(())
        })
    }

    fn check_taken_ids(&mut self, id: &ClientIdArg, user_name: &str) -> ServerResult<()> {
        if let Some(old_user_name) = self.taken_ids.get(id) {
            if user_name != old_user_name {
                return Err(ServerError::new_kind(
                    &format!("La ID <{}> se encuentra reservada por otro usuario", id),
                    ServerErrorKind::TakenID,
                ));
            } else {
                return Ok(());
            }
        }
        Ok(())
    }

    fn check_credentials(&mut self, connect: &Connect) -> ServerResult<()> {
        if connect.client_id().starts_with(GENERIC_ID_SUFFIX) {
            return Err(ServerError::new_kind(
                &format!(
                    "IDs que empiezan con {} estan reservadas por el servidor",
                    GENERIC_ID_SUFFIX
                ),
                ServerErrorKind::TakenID,
            ));
        }

        if let Some(user_name) = connect.user_name() {
            let password = login::search_password(&self.accounts_path, user_name)?;
            if password == *connect.password().unwrap_or(&String::new()) {
                self.check_taken_ids(connect.client_id(), user_name)
            } else {
                Err(ServerError::new_kind(
                    "Contraseña incorrecta",
                    ServerErrorKind::InvalidPassword,
                ))
            }
        } else {
            Err(ServerError::new_kind(
                "Clientes sin user_name no estan permitidos",
                ServerErrorKind::ClientNotInWhitelist,
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
                "Clientes con id vacia deben tener clean session en true",
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
        session: &Session,
        mut connect: Connect,
    ) -> ServerResult<ConnectInfo> {
        if connect.client_id().is_empty() {
            self.process_client_empty_id(&mut connect)?;
        }
        let id = connect.client_id().to_owned();

        let mut takeover_last_will = None;
        let session_present;
        let return_code;
        let addr = session.addr();

        // Hay una sesion_presente en el servidor con la misma ID
        if let Some(old_client) = self.clients.get_mut(&id) {
            takeover_last_will = old_client
                .get_mut()?
                .reconnect(connect, session.try_clone()?)?;
            session_present = true;
            return_code = ConnackReturnCode::Accepted;
        } else {
            let mut client = Client::new(connect);
            client.connect(session.try_clone()?)?;

            self.taken_ids.insert(
                id.to_owned(),
                client
                    .user_name()
                    .expect("Se esperaba un user_name")
                    .to_owned(),
            );
            self.client_add(client);
            session_present = false;
            return_code = ConnackReturnCode::Accepted;
        }
        self.current_addrs.insert(id.to_owned(), addr);
        Ok(ConnectInfo {
            id,
            session_present,
            return_code,
            takeover_last_will,
        })
    }

    pub fn new_session(
        &mut self,
        session: &Session,
        connect: Connect,
    ) -> ServerResult<ConnectInfo> {
        match self.check_credentials(&connect) {
            Ok(()) => self.new_session_unchecked(session, connect),
            Err(err) if err.kind() == ServerErrorKind::ClientNotInWhitelist => {
                warn!("<{}>: Usuario invalido", connect.client_id());
                Err(ServerError::new_kind(
                    "Usuario invalido",
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::NotAuthorized),
                ))
            }
            Err(err) if err.kind() == ServerErrorKind::InvalidPassword => {
                warn!("<{}>: Contraseña invalida", connect.client_id());
                Err(ServerError::new_kind(
                    &err.to_string(),
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::BadUserNameOrPassword),
                ))
            }
            Err(err) if err.kind() == ServerErrorKind::TakenID => {
                warn!(
                    "<{}>: La ID ya se encuentra en uso en el servidor",
                    connect.client_id()
                );
                Err(ServerError::new_kind(
                    &err.to_string(),
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
                ))
            }
            Err(err) => Err(err),
        }
    }

    pub fn finish_all_sessions(&mut self, gracefully: bool) -> ServerResult<()> {
        for (id, client) in &self.clients {
            debug!("<{}>: Desconectando", id);
            client.lock()?.disconnect(gracefully)?;
        }
        self.clients.retain(|_id, client| match client.get_mut() {
            Ok(client) => client.clean_session(),
            Err(_) => false,
        });
        Ok(())
    }
}
