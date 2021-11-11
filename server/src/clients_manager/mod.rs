mod login;

use std::{collections::HashMap, sync::Mutex};

use packets::publish::Publish;
use tracing::debug;

use crate::{
    client::Client,
    server::{server_error::ServerErrorKind, ServerError, ServerResult},
};

pub struct ClientsManager {
    clients: HashMap<String, Mutex<Client>>,
    /// client_id - username
    taken_ids: HashMap<String, String>,
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

    pub fn client_do<F>(&self, id: &str, action: F) -> ServerResult<()>
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

    fn client_remove(&mut self, id: &str) -> ServerResult<()> {
        self.clients.remove(id);
        Ok(())
    }

    pub fn finish_session(&mut self, id: &str, gracefully: bool) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.disconnect(gracefully);
            Ok(())
        })?;
        if self.clients.get(id).unwrap().lock()?.clean_session() {
            debug!("<{}>: Terminando sesion con clean_session = true", id);
            self.client_remove(id)?;
        } else {
            debug!("<{}>: Terminando sesion con clean_session = false", id);
        }
        Ok(())
    }

    fn client_add(&mut self, client: Client) -> ServerResult<()> {
        self.clients
            .insert(client.id().to_owned(), Mutex::new(client));
        Ok(())
    }

    pub fn send_unacknowledged(&self, id: &str) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.send_unacknowledged();
            Ok(())
        })
    }

    pub fn send_publish(&self, id: &str, publish: Publish) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.send_publish(publish);
            Ok(())
        })
    }

    fn exists(&self, id: &str) -> ServerResult<bool> {
        Ok(self.clients.contains_key(id))
    }

    fn check_taken_ids(&mut self, id: &str, user_name: &str) -> ServerResult<()> {
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
            if password == *client.password().expect("Se esperaba una contraseña") {
                self.check_taken_ids(client.id(), user_name)
            } else {
                Err(ServerError::new_kind(
                    "Contraseña invalida",
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
        // Hay una sesion_presente en el servidor con la misma ID
        // (con clean_sesion = false)
        let id = client.id().to_owned();
        if self.exists(client.id())? {
            // El nuevo cliente tiene clean_sesion = true, descarto
            // sesion vieja
            if client.clean_session() {
                self.client_remove(client.id())?;
                client.send_connack(0, 0)?;
                self.client_add(client)?;
            }
            // El cliente quiere reconectarse a la sesion guaradada
            else {
                client.send_connack(0, 0)?;
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
            client.send_connack(0, 0)?;
            self.client_add(client)?;
        }
        Ok(())
    }

    pub fn new_session(&mut self, mut client: Client) -> ServerResult<()> {
        match self.check_credentials(&client) {
            Ok(()) => self.new_session_unchecked(client),
            Err(err) if err.kind() == ServerErrorKind::ClientNotInWhitelist => {
                client.send_connack(0, 5)?;
                Ok(())
            }
            Err(err) if err.kind() == ServerErrorKind::InvalidPassword => {
                client.send_connack(0, 4)?;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn is_connected(&self, id: &str) -> ServerResult<bool> {
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
            client.lock().unwrap().disconnect(gracefully);
        }
        self.clients
            .retain(|_id, client| !client.get_mut().unwrap().clean_session());
        Ok(())
    }

    pub fn keep_alive(&self, id: &str) -> ServerResult<u16> {
        let mut keep_alive = 0;
        self.client_do(id, |client| {
            keep_alive = client.keep_alive();
            Ok(())
        })?;
        Ok(keep_alive)
    }
}
