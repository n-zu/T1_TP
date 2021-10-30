use std::{
    collections::HashMap,
    sync::{Mutex, RwLock},
};

use super::Client;

use super::{Publish, ServerError, ServerErrorKind};

pub struct Clients {
    clients: RwLock<HashMap<String, Mutex<Client>>>,
}

impl Clients {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }

    pub fn new_client(&self, client: Client) -> Result<(), ServerError> {
        match self
            .clients
            .write()
            .unwrap()
            .insert(client.id().to_owned(), Mutex::new(client))
        {
            None => Ok(()),
            Some(old_client) => Err(ServerError::new_kind(
                "Se encontro un cliente con la misma id",
                ServerErrorKind::RepeatedId,
            )),
        }
    }

    pub fn disconnect(&self, client_id: &str) {
        self.clients
            .read()
            .unwrap()
            .get(client_id)
            .unwrap()
            .lock()
            .unwrap()
            .disconnect()
    }

    pub fn is_alive(&self, client_id: &str) -> Result<bool, ServerError> {
        match self.clients.read().unwrap().get(client_id) {
            None => Err(ServerError::new_msg("No se encontro la id")),
            Some(client) => Ok(client.lock().unwrap().alive())
        }
    }

    pub fn send_publish(&self, client_id: &str, publish: Publish) {
        self.clients
            .read()
            .unwrap()
            .get(client_id)
            .unwrap()
            .lock()
            .unwrap()
            .write_all(&publish.encode())
            .unwrap()
    }

    pub fn send_connack(&self, client_id: &str) {
        let response = self
            .clients
            .read()
            .unwrap()
            .get(client_id)
            .unwrap()
            .lock()
            .unwrap()
            .connect()
            .response()
            .clone();
        self.clients
            .read()
            .unwrap()
            .get(client_id)
            .unwrap()
            .lock()
            .unwrap()
            .write_all(&response.encode())
            .unwrap();
    }

    pub fn remove(&self, client_id: &str) {
        self.clients.write().unwrap().remove(client_id).unwrap();
    }
}
