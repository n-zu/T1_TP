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

pub struct ClientsManager {
    clients: HashMap<String, Mutex<Client>>,
    /// client_id - username
    taken_ids: HashMap<ClientId, Username>,
    accounts_path: String,
}

impl ClientsManager {
    pub fn new(accounts_path: &str) -> Self {
        Self {
            clients: HashMap::new(),
            taken_ids: HashMap::new(),
            accounts_path: accounts_path.to_owned(),
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
                "No existe el cliente",
                ServerErrorKind::ClientNotFound,
            )),
        }
    }

    pub fn finish_session(&mut self, id: &ClientIdArg, gracefully: bool) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.disconnect(gracefully);
            Ok(())
        })?;
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
        } else {
            debug!("<{}>: Terminando sesion con clean_session = false", id);
        }
        Ok(())
    }

    fn client_add(&mut self, client: Client) -> Option<Mutex<Client>> {
        self.clients
            .insert(client.id().to_owned(), Mutex::new(client))
    }

    pub fn send_unacknowledged(&self, id: &ClientIdArg) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.send_unacknowledged();
            Ok(())
        })
    }

    pub fn send_publish(&self, id: &ClientIdArg, publish: Publish) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.send_publish(publish);
            Ok(())
        })
    }

    fn exists(&self, id: &ClientIdArg) -> ServerResult<bool> {
        Ok(self.clients.contains_key(id))
    }

    fn check_taken_ids(&mut self, id: &ClientIdArg, user_name: &str) -> ServerResult<()> {
        if let Some(old_user_name) = self.taken_ids.get(id) {
            if user_name != old_user_name {
                return Err(ServerError::new_kind(
                    &format!("La ID <{}> se encuentra reservada por otro usuario", id),
                    ServerErrorKind::TakenID,
                ));
            } else {
                Ok(())
            }
        } else {
            self.taken_ids.insert(id.to_owned(), user_name.to_owned());
            Ok(())
        }
    }

    fn check_credentials(&mut self, client: &Client) -> ServerResult<()> {
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

    fn new_session_unchecked(&mut self, mut client: Client) -> ServerResult<()> {
        let id = client.id().to_owned();
        // Hay una sesion_presente en el servidor con la misma ID
        // (con clean_sesion = false)
        if self.exists(client.id())? {
            // Descarto sesion vieja
            if client.clean_session() {
                self.clients.remove(client.id());
                client.send_connack(Connack::new(false, ConnackReturnCode::Accepted))?;
                self.client_add(client);
            }
            // El cliente quiere reconectarse a la sesion guaradada
            else {
                client.send_connack(Connack::new(true, ConnackReturnCode::Accepted))?;
                self.client_do(&id, |mut old_client| {
                    old_client.reconnect(client)?;
                    Ok(())
                })?;
                self.client_do(&id, |mut client| {
                    client.send_unacknowledged();
                    Ok(())
                })?;
            }
        } else {
            client.send_connack(Connack::new(false, ConnackReturnCode::Accepted))?;
            self.client_add(client);
        }
        Ok(())
    }

    pub fn new_session(&mut self, mut client: Client) -> ServerResult<()> {
        match self.check_credentials(&client) {
            Ok(()) => self.new_session_unchecked(client),
            Err(err) if err.kind() == ServerErrorKind::ClientNotInWhitelist => {
                warn!("<{}>: Usuario invalido", client.id());
                client.send_connack(Connack::new(false, ConnackReturnCode::NotAuthorized))?;
                Err(err)
            }
            Err(err) if err.kind() == ServerErrorKind::InvalidPassword => {
                warn!("<{}>: Contraseña invalida", client.id());
                client.send_connack(Connack::new(
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
                client.send_connack(Connack::new(false, ConnackReturnCode::IdentifierRejected))?;
                Err(err)
            }
            Err(err) => Err(err),
        }
    }

    pub fn is_connected(&self, id: &ClientIdArg) -> ServerResult<bool> {
        let mut alive = false;
        match self.client_do(id, |client| {
            alive = client.connected();
            Ok(())
        }) {
            Ok(_) => Ok(alive),
            Err(err) if err.kind() == ServerErrorKind::ClientNotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub fn finish_all_sessions(&mut self, gracefully: bool) -> ServerResult<()> {
        for (id, client) in &self.clients {
            debug!("<{}>: Desconectando", id);
            client.lock()?.disconnect(gracefully);
        }
        self.clients.retain(|_id, client| match client.get_mut() {
            Ok(client) => client.clean_session(),
            Err(_) => false,
        });
        Ok(())
    }

    pub fn keep_alive(&self, id: &ClientIdArg) -> ServerResult<u16> {
        let mut keep_alive = 0;
        self.client_do(id, |client| {
            keep_alive = client.keep_alive();
            Ok(())
        })?;
        Ok(keep_alive)
    }
}
