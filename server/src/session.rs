#![allow(dead_code)]

use std::{collections::HashMap, sync::Mutex};

use packets::{puback::Puback, publish::Publish};

use crate::{
    client::Client,
    server::{server_error::ServerErrorKind, ServerError, ServerResult},
    server_packets::Connack,
};

pub struct Session {
    clients: HashMap<String, Mutex<Client>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
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

    pub fn disconnect(&self, id: &str, gracefully: bool) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.disconnect(gracefully);
            Ok(())
        })
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

    pub fn send_puback(&self, id: &str, puback: &Puback) -> ServerResult<()> {
        self.client_do(&id, |mut client| {
            client.write_all(&puback.encode()).unwrap();
            Ok(())
        })
    }

    pub fn send_publish(&self, id: &str, publish: Publish) -> ServerResult<()> {
        self.client_do(&id, |mut client| {
            client.send_publish(publish);
            Ok(())
        })
    }

    fn exists(&self, id: &str) -> ServerResult<bool> {
        Ok(self.clients.contains_key(id))
    }

    fn clean_session(&self, id: &str) -> ServerResult<bool> {
        let mut clean_session = false;
        self.client_do(id, |client| {
            clean_session = client.clean_session();
            Ok(())
        })?;
        Ok(clean_session)
    }

    fn send_connack(&self, id: &str, session_present: u8, return_code: u8) -> ServerResult<()> {
        self.client_do(id, |mut client| {
            client.write_all(&Connack::new(session_present, return_code).encode())?;
            Ok(())
        })
    }

    pub fn connect(&mut self, client: Client) -> ServerResult<()> {
        // Hay una sesion_presente en el servidor con la misma ID
        // (con clean_sesion = false)
        let id = client.id().to_owned();
        if self.exists(client.id())? {
            // El nuevo cliente tiene clean_sesion = true, descarto
            // sesion vieja
            if client.clean_session() {
                self.client_remove(client.id())?;
                self.client_add(client)?;
                self.send_connack(&id, 0, 0)?;
            }
            // El cliente quiere reconectarse a la sesion guaradada
            else {
                self.client_do(&id, |mut old_client| {
                    old_client.reconnect(client)?;
                    Ok(())
                })?;
                self.send_connack(&id, 1, 0)?;
            }
        } else {
            self.client_add(client)?;
            self.send_connack(&id, 0, 0)?;
        }

        Ok(())
    }

    pub fn connected(&self, id: &str) -> ServerResult<bool> {
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

    pub fn keep_alive(&self, id: &str) -> ServerResult<u16> {
        let mut keep_alive = 0;
        self.client_do(id, |client| {
            keep_alive = client.keep_alive();
            Ok(())
        })?;
        Ok(keep_alive)
    }

    pub fn finish_session(&mut self, id: &str) -> ServerResult<()> {
        if self.clients.get(id).unwrap().lock()?.clean_session() {
            self.client_remove(id)?;
        }
        Ok(())
    }
}
