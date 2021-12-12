mod login;

use core::fmt;
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::Mutex,
};

use packets::{connack::ConnackReturnCode, connect::Connect, publish::Publish};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    client::Client,
    network_connection::NetworkConnection,
    server::{server_error::ServerErrorKind, ClientId, ClientIdArg, ServerError, ServerResult},
    traits::Close,
};

const GENERIC_ID_SUFFIX: &str = "__CLIENT__";

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientsManager<S, I>
where
    S: Read + Write + Send + Sync + 'static,
    I: fmt::Display,
{
    #[serde(bound(serialize = "Client<S, I>: Serialize<>"))]
    #[serde(bound(deserialize = "Client<S, I>: Deserialize<'de>"))]
    clients: HashMap<ClientId, Mutex<Client<S, I>>>,
    accounts_path: Option<String>,
    generic_ids_counter: u32,
}

#[derive(Debug)]
pub struct DisconnectInfo {
    pub publish_last_will: Option<Publish>,
    pub clean_session: bool,
}

#[derive(Debug, PartialEq)]
pub struct ConnectInfo {
    pub id: ClientId,
    pub session_present: bool,
    pub return_code: ConnackReturnCode,
    pub takeover_last_will: Option<Publish>,
}

#[derive(Debug)]
pub struct ShutdownInfo {
    pub clean_session_ids: Vec<ClientId>,
    pub last_will_packets: Vec<(ClientId, Publish)>,
}

impl<S, I> ClientsManager<S, I>
where
    S: Read + Write + Send + Sync + 'static,
    I: fmt::Display + Clone + std::hash::Hash + Eq,
{
    pub fn new(accounts_path: Option<&str>) -> Self {
        Self {
            clients: HashMap::new(),
            accounts_path: accounts_path.map(|x| x.to_owned()),
            generic_ids_counter: 0,
        }
    }

    pub fn client_do<F>(&self, id: &ClientIdArg, action: F) -> ServerResult<()>
    where
        F: FnOnce(std::sync::MutexGuard<'_, Client<S, I>>) -> ServerResult<()>,
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
        network_connection: NetworkConnection<S, I>,
        gracefully: bool,
    ) -> ServerResult<DisconnectInfo>
    where
        S: Close,
    {
        // Chequeo si ya fue desconectado por el proceso
        // de Client Take-Over
        let old_id = self.get_client_property(id, |client| Ok(client.connection_id().cloned()))?;
        if let Some(old_id) = old_id {
            if *network_connection.id() != old_id {
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

    fn client_add(&mut self, client: Client<S, I>) -> Option<Mutex<Client<S, I>>> {
        self.clients
            .insert(client.id().to_owned(), Mutex::new(client))
    }

    fn check_taken_ids(&mut self, id: &ClientIdArg, user_name: &str) -> ServerResult<()> {
        let client = match self.clients.get(id) {
            Some(client) => client.lock()?,
            None => return Ok(()),
        };
        let old_user_name = match client.user_name() {
            Some(user_name) => user_name,
            None => return Ok(()),
        };

        if user_name != old_user_name {
            Err(ServerError::new_kind(
                &format!("La ID <{}> se encuentra reservada por otro usuario", id),
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
            ))
        } else {
            Ok(())
        }
    }

    fn check_credentials(&mut self, connect: &Connect) -> ServerResult<()> {
        if connect.client_id().starts_with(GENERIC_ID_SUFFIX) {
            return Err(ServerError::new_kind(
                "ID con prefijo invalido",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
            ));
        }

        let accounts_path = match &self.accounts_path {
            None => return Ok(()),
            Some(path) => path,
        };
        let user_name = match connect.user_name() {
            Some(user_name) => user_name,
            None => {
                return Err(ServerError::new_kind(
                    "Clientes sin user_name no estan permitidos",
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::NotAuthorized),
                ))
            }
        };
        let password = login::search_password(accounts_path, user_name)?;
        if password == *connect.password().unwrap_or(&String::new()) {
            self.check_taken_ids(connect.client_id(), user_name)
        } else {
            Err(ServerError::new_kind(
                "Contraseña incorrecta",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::BadUserNameOrPassword),
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

    pub fn new_session(
        &mut self,
        network_connection: NetworkConnection<S, I>,
        mut connect: Connect,
    ) -> ServerResult<ConnectInfo>
    where
        S: Close,
    {
        self.check_credentials(&connect)?;

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
                .reconnect(connect, network_connection)?;
            session_present = true;
        } else {
            let client = Client::new(connect, network_connection);
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

    pub fn shutdown(&mut self, gracefully: bool) -> ServerResult<ShutdownInfo>
    where
        S: Close,
    {
        let mut clean_session_ids = vec![];
        let mut last_will_packets = vec![];

        for (id, client) in &self.clients {
            if let Some(last_will) = client.lock()?.disconnect(gracefully)? {
                last_will_packets.push((id.to_owned(), last_will));
            }
        }

        self.clients.retain(|_id, client| match client.get_mut() {
            Ok(client) => {
                if client.clean_session() {
                    clean_session_ids.push(client.id().to_owned());
                    false
                } else {
                    true
                }
            }
            Err(_) => false,
        });
        Ok(ShutdownInfo {
            clean_session_ids,
            last_will_packets,
        })
    }
}

#[cfg(test)]
mod tests;
