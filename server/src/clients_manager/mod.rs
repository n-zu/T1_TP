pub mod simple_login;

use core::fmt;
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::Mutex,
    vec,
};

use packets::{connack::ConnackReturnCode, connect::Connect, publish::Publish};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::{
    client::Client,
    network_connection::NetworkConnection,
    server::{server_error::ServerErrorKind, ClientId, ClientIdArg, ServerError, ServerResult},
    traits::{Close, Login, LoginResult, Interrupt},
};

const GENERIC_ID_SUFFIX: &str = "__CLIENT__";

/// Structure that manages the clients of the server.
/// This includes connecting, reconneting, disconnecting
/// and authenticating clients
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientsManager<S, I>
where
    S: Write + Interrupt + Send + Sync + 'static,
    I: fmt::Display,
{
    #[serde(bound(serialize = "Client<S, I>: Serialize<>"))]
    #[serde(bound(deserialize = "Client<S, I>: Deserialize<'de>"))]
    /// All persistent client information.
    /// When a disconnection occurs from a client
    /// with clean session set to true, the client
    /// information is deleted (that is, it is removed
    /// from the HashMap)
    clients: HashMap<ClientId, Mutex<Client<S, I>>>,
    #[serde(skip, default = "Default::default")]
    /// Login method. If None, clients can connect
    /// without authentication
    login: Option<Box<dyn Login>>,
    /// Keeps track of how many clients without
    /// client_id are connected. Useful to assign
    /// them a unique default ID
    generic_ids_counter: u32,
}

/// Information related to the disconnection
/// of a client
#[derive(Debug)]
pub struct DisconnectInfo {
    /// It is the Last Will packet to be published
    /// If the client did not specify it in his
    /// last connection, it is None
    pub publish_last_will: Option<Publish>,
    /// clean_session value specified by the client
    /// when connecting. Useful to delete customer
    /// information that is not stored by the
    /// [ClientsManager]
    pub clean_session: bool,
}

/// Informtation related to the connection of a client.
///
/// It is only associated with a valid and accepted
/// connection (that is, if a method returns this
/// structure, it means that the client was connected
/// correctly)
#[derive(Debug, PartialEq)]
pub struct ConnectInfo {
    /// ID of the client. Usually specified by the
    /// client in the [`Connect`] packet. If not
    /// specified, the [`ClientsManager`] will assign one
    pub id: ClientId,
    /// Indicates if there was a previous session
    /// present on the server associated with the
    /// client_id. This session could be active (in
    /// case of a client takeover) or not (in case
    /// a client connect with clean_session false and
    /// then disconnects)
    pub session_present: bool,
    /// Last Will packet specified in the client's
    /// previous session, in the event of a takeover
    /// reconnection. If this does not happen, or the
    /// client did not specify a LastWill packet, it
    /// is None
    pub takeover_last_will: Option<Publish>,
}

#[derive(Debug)]
pub struct ShutdownInfo {
    pub clean_session_ids: Vec<ClientId>,
    pub last_will_packets: Vec<(ClientId, Publish)>,
}

impl<S, I> ClientsManager<S, I>
where
    S: Read + Interrupt + Write + Send + Sync + 'static,
    I: fmt::Display + Clone + std::hash::Hash + Eq,
{
    /// Creates a new [`ClientsManager`], without any
    /// client connected
    pub fn new(login: Option<Box<dyn Login>>) -> Self {
        Self {
            clients: HashMap::new(),
            login,
            generic_ids_counter: 0,
        }
    }

    /// Locks a client and applies the received function to it.
    /// If the client does not exist, it returns an error of
    /// kind [`ServerErrorKind::ClientNotFound`]
    pub fn client_do<F, T>(&self, id: &ClientIdArg, action: F) -> ServerResult<T>
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

    /// Tries to disconnect a client. If the client specified
    /// clean_session to false, its information is kept
    /// in (self.clients). Otherwise, it is deleted.
    ///
    /// Returns the disconnection information of the client.
    ///
    /// It is improtant to note that this method does not fail
    /// if the client has clean_session false and was already
    /// disconnected, but returns a [`DisconnectInfo`] with
    /// publish_las_will in None and clean_session in false
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
        let old_id = match self.client_do(id, |client| Ok(client.connection_id().cloned())) {
            Ok(old_id) => old_id,
            Err(e) if e.kind() == ServerErrorKind::ClientNotFound => {
                return Ok(DisconnectInfo {
                    publish_last_will: None,
                    clean_session: false,
                })
            }
            Err(e) => return Err(e),
        };
        if let Some(old_id) = old_id {
            if *network_connection.id() != old_id {
                return Ok(DisconnectInfo {
                    publish_last_will: None,
                    clean_session: false,
                });
            }
        }

        let publish_last_will = self.client_do(id, |mut session| session.disconnect(gracefully))?;
        let clean_session;
        // Si la funcion anterior no devolvio error, entonces existe el cliente
        if self
            .clients
            .get(id)
            .expect("Error inesperado: no se encontro el cliente en el HashMap")
            .lock()?
            .clean_session()
        {
            self.clients.remove(id);
            clean_session = true;
        } else {
            clean_session = false;
        }
        Ok(DisconnectInfo {
            publish_last_will,
            clean_session,
        })
    }

    /// Adds a client to the client list, without doing any checks
    fn client_add(&mut self, client: Client<S, I>) -> Option<Mutex<Client<S, I>>> {
        self.clients
            .insert(client.id().to_owned(), Mutex::new(client))
    }

    /// Checks that a client_id is not reserved by another
    /// user. That is, if ther is a session with the specified id,
    /// the associated user must match the new user_name
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

    /// Checks that the [`Connect`] packet received from the client
    /// contains valid credentials. Performs the authentication
    /// (login) if a method was specified, and verifies that the
    /// rest of the fields are valid.
    ///
    /// If it could not be connected, but it corresponds to
    /// send a Connack to the client, it returns an error of kind
    /// [`ServerErrorKind::ConnectionRefused`]
    fn check_credentials(&mut self, connect: &Connect) -> ServerResult<()> {
        if connect.client_id().starts_with(GENERIC_ID_SUFFIX) {
            return Err(ServerError::new_kind(
                "ID con prefijo invalido",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::IdentifierRejected),
            ));
        }

        // No precisamos chequear las ids tomadas con check_taken_ids
        // porque en modo sin autenticacion cualquier cliente puede
        // hacer TakeOver
        let login = match &mut self.login {
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
        let password = match connect.password() {
            Some(password) => password,
            None => {
                return Err(ServerError::new_kind(
                    "Clientes sin contraseña no estan permitidos",
                    ServerErrorKind::ConnectionRefused(ConnackReturnCode::BadUserNameOrPassword),
                ))
            }
        };
        match login.login(user_name, password)? {
            LoginResult::UsernameNotFound => Err(ServerError::new_kind(
                "Usuario invalido",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::NotAuthorized),
            )),
            LoginResult::InvalidPassword => Err(ServerError::new_kind(
                "Contraseña invalida",
                ServerErrorKind::ConnectionRefused(ConnackReturnCode::BadUserNameOrPassword),
            )),
            LoginResult::Accepted => self.check_taken_ids(connect.client_id(), user_name),
        }
    }

    /// Creates a new generic ID. Guarantees that this id
    /// is unique
    fn new_generic_id(&mut self) -> String {
        self.generic_ids_counter += 1;
        let mut generic_id = String::from(GENERIC_ID_SUFFIX);
        generic_id.push_str(&self.generic_ids_counter.to_string());
        generic_id
    }

    /// Makes the necessary modifications in the [`Connect`] packet to
    /// be able to create a client from its information. If it contains
    /// invalid information, it returns an error of kinf
    /// [`ServerErrorKind::ConnectionRefused`]
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

    /// Established a new connection with a client.
    ///
    /// Returns a [`ConnectInfo`], with the information
    /// related to the connection (or reconnection) of
    /// the client.
    ///
    /// If a client with the same id was already connected,
    /// it disconnects it (takeover) and returns the Last
    /// Will, if it was specified in the previous session.
    ///
    /// Performs all the necessary checks to ensure that
    /// the session is valid
    #[instrument(skip(self, network_connection, connect) fields(socket_addr = %network_connection.id(), client_id = %connect.client_id()))]
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

        // Hay una sesion_presente en el servidor con la misma ID
        if let Some(old_client) = self.clients.get(&id) {
            info!("Reconectando");
            takeover_last_will = old_client.lock()?.reconnect(connect, network_connection)?;
            session_present = true;
        } else {
            let client = Client::new(connect, network_connection);
            self.client_add(client);
            session_present = false;
        }
        Ok(ConnectInfo {
            id,
            session_present,
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
