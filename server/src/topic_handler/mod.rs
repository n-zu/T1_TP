use std::{
    collections::{HashMap, HashSet},
    sync::RwLock,
};

pub mod topic_handler_error;
use self::topic_handler_error::TopicHandlerError;

/* DEFINICIONES TEMPORALES (la idea es después importarlas) */
pub struct Server;

pub struct Publish;
pub struct Subscribe;
pub struct Unsubscribe;

/************************************************************/

type Subtopics = HashMap<String, Topic>;
type Subscribers = HashSet<String>;

pub trait Publisher {
    fn send_publish(&self, user_id: &str, packet: &Publish);
}

const SEP: &str = "/";

pub struct TopicHandler {
    root: Topic,
}

struct Topic {
    subtopics: RwLock<Subtopics>,
    subscribers: RwLock<Subscribers>,
}

impl Topic {
    fn new() -> Self {
        Topic {
            subtopics: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashSet::new()),
        }
    }
}

impl TopicHandler {
    pub fn subscribe(&self, _packet: Subscribe, client_id: &str) -> Result<(), TopicHandlerError> {
        let topic = "todo";

        subscribe_rec(&self.root, topic, client_id)?;

        Ok(())
    }

    pub fn publish(
        &self,
        packet: Publish,
        server: impl Publisher,
    ) -> Result<(), TopicHandlerError> {
        let full_topic = "todo";
        let mut subs: Subscribers = HashSet::new();

        get_subs_rec(&self.root, full_topic, &mut subs)?;

        for sub in subs {
            server.send_publish(&sub, &packet);
        }
        Ok(())
    }

    pub fn unsubscribe(
        &self,
        _packet: Unsubscribe,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        let full_topic = "todo";
        unsubscribe_rec(&self.root, full_topic, client_id)?;
        Ok(())
    }

    pub fn new() -> Self {
        Self { root: Topic::new() }
    }

    fn remove_client(&self, client_id: &str) -> Result<(), TopicHandlerError> {
        remove_client_rec(&self.root, client_id)?;
        Ok(())
    }
}

// Lo tuve que hacer recursivo porque sino era un caos el tema de mantener todos los
// locks desbloqueados, ya que no los podia dropear porque perdia las referencias internas
fn get_subs_rec(
    node: &Topic,
    topic_name: &str,
    subs: &mut Subscribers,
) -> Result<(), TopicHandlerError> {
    match topic_name.split_once(SEP) {
        // Aca se le puede agregar tratamiento especial para *, #, etc.
        // Uso un HashMap para no preocuparse por agregar el mismo cliente mas de una vez
        Some((topic_name, rest)) => {
            let subtopics = node.subtopics.read()?;
            if let Some(subtopic) = subtopics.get(topic_name) {
                get_subs_rec(subtopic, rest, subs)?;
            }
        }
        None => {
            let subscribers = node.subscribers.read()?;
            subs.extend(subscribers.iter().cloned());
        }
    }

    Ok(())
}

fn subscribe_rec(node: &Topic, topic_name: &str, user_id: &str) -> Result<(), TopicHandlerError> {
    match topic_name.split_once(SEP) {
        // Aca se le puede agregar tratamiento especial para *, #, etc.
        Some((topic_name, rest)) => {
            let mut subtopics = node.subtopics.read()?;

            // Insercion de nuevo nodo
            if subtopics.get(topic_name).is_none() {
                drop(subtopics); //lo tengo que pedir en modo write y si esta leyendo no va a poder
                let mut wr_subtopics = node.subtopics.write()?;
                let subtopic = Topic::new();
                wr_subtopics.insert(topic_name.to_string(), subtopic);
                drop(wr_subtopics);
                subtopics = node.subtopics.read()?;
            }

            // En principio siempre tiene que entrar a este if, pero lo pongo para no unwrappear
            if let Some(subtopic) = subtopics.get(topic_name) {
                subscribe_rec(subtopic, rest, user_id)?;
            }
        }
        None => {
            node.subscribers.write()?.insert(user_id.to_string());
        }
    }

    Ok(())
}

fn unsubscribe_rec(node: &Topic, topic_name: &str, user_id: &str) -> Result<(), TopicHandlerError> {
    match topic_name.split_once(SEP) {
        // Aca se le puede agregar tratamiento especial para *, #, etc.
        Some((topic_name, rest)) => {
            let subtopics = node.subtopics.read()?;

            if let Some(subtopic) = subtopics.get(topic_name) {
                unsubscribe_rec(subtopic, rest, user_id)?;
            }
        }
        None => {
            node.subscribers.write()?.remove(user_id);
        }
    }

    // si falla no importa
    let _res = clean_node(node);
    Ok(())
}

fn clean_node(node: &Topic) -> Result<(), TopicHandlerError> {
    let mut empty_subtopics = Vec::new();

    let subtopics_read = node.subtopics.read()?;
    for (sub_topic, topic) in subtopics_read.iter() {
        if topic.subscribers.read()?.is_empty() && topic.subtopics.read()?.is_empty() {
            empty_subtopics.push(sub_topic.to_string());
        }
    }

    if !empty_subtopics.is_empty() {
        drop(subtopics_read);
        let mut subtopics_write = node.subtopics.write()?;
        for sub_topic in empty_subtopics {
            subtopics_write.remove(&sub_topic);
        }
    }

    Ok(())
}

fn remove_client_rec(node: &Topic, user_id: &str) -> Result<(), TopicHandlerError> {
    for subtopic in node.subtopics.read()?.values() {
        remove_client_rec(subtopic, user_id)?;
    }

    let subs_read = node.subscribers.read()?;
    if subs_read.contains(user_id) {
        drop(subs_read);
        node.subscribers.write()?.remove(user_id);
    }

    // si falla no importa
    let _res = clean_node(node);
    Ok(())
}
