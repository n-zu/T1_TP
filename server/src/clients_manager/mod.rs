mod login;

use std::{collections::HashMap, sync::Mutex};

use packets::{
    connack::{Connack, ConnackReturnCode},
    publish::Publish,
};
use tracing::{debug, warn};

use crate::{
    client::Client,
    server::{server_error::ServerErrorKind, ClientId, ClientIdArg, ServerError, ServerResult},
};

type Username = String;
const GENERIC_ID_SUFFIX: &str = "__CLIENT__";

pub struct ClientsManager {
    clients: HashMap<String, Mutex<Client>>,
    /// client_id - username
    taken_ids: HashMap<ClientId, Username>,
    accounts_path: String,
    generic_ids_counter: u32,
}

pub struct DisconnectInfo {
    pub publish_last_will: Option<Publish>,
    pub clean_session: bool,
}

pub struct ConnectInfo {
    pub id: ClientId,
    pub takeover_last_will: Option<Publish>,
}

impl ClientsManager {
    pub fn new(accounts_path: &str) -> Self {
        Self {
            clients: HashMap::new(),
            taken_ids: HashMap::new(),
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
        gracefully: bool,
    ) -> ServerResult<DisconnectInfo> {
        let publish_last_will = self.get_client_property(id, |mut client| {
            client.disconnect(gracefully)
        })?;
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
            client.send_publish(publish);
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

    fn check_credentials(&mut self, client: &Client) -> ServerResult<()> {
        if client.id().starts_with(GENERIC_ID_SUFFIX) {
            return Err(ServerError::new_kind(
                &format!(
                    "IDs que empiezan con {} estan reservadas por el servidor",
                    GENERIC_ID_SUFFIX
                ),
                ServerErrorKind::TakenID,
            ));
        }

        if let Some(user_name) = client.user_name() {
            let password = login::search_password(&self.accounts_path, user_name)?;
            if password == *client.password().unwrap_or(&String::new()) {
                self.check_taken_ids(client.id(), user_name)
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

    fn process_client_empty_id(&mut self, client: &mut Client) -> ServerResult<()> {
        if !client.clean_session() {
            client.send_packet(Connack::new(false, ConnackReturnCode::IdentifierRejected))?;
            Err(ServerError::new_kind(
                "Clientes con id vacia deben tener clean session en true",
                ServerErrorKind::ProtocolViolation,
            ))
        } else {
            let id = self.new_generic_id();
            client.set_id(&id)?;
            Ok(())
        }
    }

    fn new_session_unchecked(&mut self, mut client: Client) -> ServerResult<ConnectInfo> {
        if client.id().is_empty() {
            self.process_client_empty_id(&mut client)?;
        }
        let id = client.id().to_owned();

        let mut takeover_last_will = None;
        // Hay una sesion_presente en el servidor con la misma ID
        if let Some(old_client) = self.clients.get_mut(&id) {
            client.send_packet(Connack::new(true, ConnackReturnCode::Accepted))?;
            takeover_last_will = old_client.lock()?.reconnect(client)?;
        } else {
            self.taken_ids.insert(
                id.to_owned(),
                client
                    .user_name()
                    .expect("Se esperaba un user_name")
                    .to_owned(),
            );
            client.send_packet(Connack::new(false, ConnackReturnCode::Accepted))?;
            self.client_add(client);
        }
        Ok(ConnectInfo {
            id,
            takeover_last_will,
        })
    }

    pub fn new_session(&mut self, mut client: Client) -> ServerResult<ConnectInfo> {
        match self.check_credentials(&client) {
            Ok(()) => self.new_session_unchecked(client),
            Err(err) if err.kind() == ServerErrorKind::ClientNotInWhitelist => {
                warn!("<{}>: Usuario invalido", client.id());
                client.send_packet(Connack::new(false, ConnackReturnCode::NotAuthorized))?;
                Err(err)
            }
            Err(err) if err.kind() == ServerErrorKind::InvalidPassword => {
                warn!("<{}>: Contraseña invalida", client.id());
                client.send_packet(Connack::new(
                    false,
                    ConnackReturnCode::BadUserNameOrPassword,
                ))?;
                Err(err)
            }
            Err(err) if err.kind() == ServerErrorKind::TakenID => {
                warn!(
                    "<{}>: La ID ya se encuentra en uso en el servidor",
                    client.id()
                );
                client.send_packet(Connack::new(false, ConnackReturnCode::IdentifierRejected))?;
                Err(err)
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
