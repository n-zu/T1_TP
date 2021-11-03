#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Mutex, RwLock},
};

use crate::{
    client::Client,
    server::{server_error::ServerErrorKind, ServerError, ServerResult},
    server_packets::Connack,
};

type ClientId = String;

pub struct Session {
    clients: RwLock<HashMap<ClientId, Mutex<Client>>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }

    pub fn client_do<F>(&self, id: &ClientId, action: F) -> ServerResult<()>
    where
        F: FnOnce(std::sync::MutexGuard<'_, Client>) -> (),
    {
        match self.clients.read().expect("Lock envenenado").get(id) {
            Some(client) => {
                action(client.lock().expect("Lock envenenado"));
                Ok(())
            }
            None => Err(ServerError::new_kind(
                "No existe el cliente",
                ServerErrorKind::ClientNotFound,
            )),
        }
    }

    fn client_remove(&self, id: &ClientId) -> ServerResult<()> {
        self.clients
            .write()
            .expect("Lock envenenado")
            .remove(id)
            .unwrap();
        Ok(())
    }

    pub fn disconnect(&self, id: &ClientId) -> ServerResult<()> {
        self.client_do(id, |client| client.disconnect())
    }

    fn client_add(&self, client: Client) {
        self.clients
            .write()
            .expect("Lock envenenado")
            .insert(client.id().to_owned(), Mutex::new(client));
    }

    fn exists(&self, id: &ClientId) -> bool {
        self.clients
            .read()
            .expect("Lock envenenado")
            .contains_key(id)
    }

    fn clean_session(&self, id: &ClientId) -> bool {
        let mut clean_session = false;
        self.client_do(id, |client| clean_session = client.clean_session())
            .unwrap();
        clean_session
    }

    fn send_connack(&self, id: &ClientId, session_present: u8, return_code: u8) {
        self.client_do(id, |mut client| {
            client
                .write_all(&Connack::new(session_present, return_code).encode())
                .unwrap();
        })
        .unwrap();
    }

    fn restore_session(&self, id: &ClientId) {
        self.client_do(id, |mut client| {
            client.send_unacknowledged();
        })
        .unwrap();
    }

    pub fn connect(&self, client: Client) -> ServerResult<()> {
        // Hay una sesion_presente en el servidor con la misma ID
        // (con clean_sesion = false)
        if self.exists(client.id()) {
            // El nuevo cliente tiene clean_sesion = true, descarto
            // sesion vieja
            let id = client.id().to_owned();
            if client.clean_session() {
                self.client_remove(client.id())?;
                self.client_add(client);
                self.send_connack(&id, 0, 0);
            }
            // El cliente quiere reconectarse a la sesion guaradada
            else {
                self.client_do(&id, |mut old_client| {
                    old_client.reconnect(client).unwrap();
                })
                .unwrap();
                self.send_connack(&id, 1, 0);
                self.restore_session(&id);
            }
        } else {
            let id = client.id().to_owned();
            self.client_add(client);
            self.send_connack(&id, 0, 0);
        }

        Ok(())
    }

    pub fn connected(&self, id: &ClientId) -> bool {
        let mut alive = false;
        match self.client_do(id, |client| alive = client.connected()) {
            Ok(_) => alive,
            Err(_) => false,
        }
    }

    pub fn refresh(&self, id: &ClientId) {
        self.client_do(id, |mut client| client.refresh()).unwrap()
    }

    pub fn finish_session(&self, id: &ClientId) {
        let clients = self.clients.read().unwrap();
        let client = clients.get(id).unwrap().lock().unwrap();
        if client.clean_session() {
            drop(client);
            drop(clients);
            self.client_remove(id).unwrap();
        }
    }
}
