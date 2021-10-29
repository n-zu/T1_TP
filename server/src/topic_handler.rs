use std::{collections::{HashMap, HashSet, hash_set::Iter}, error::Error, fmt::Display, sync::{PoisonError, RwLock, RwLockReadGuard}};

/* DEFINICIONES TEMPORALES (la idea es después importarlas) */
pub struct Server;

pub struct Publish;
pub struct Subscribe;
pub struct Unsubscribe;

impl Server {
    fn send_publish(&self, packet : &Publish, user_id : &str) {
        todo!("pendiente");
    }
}

/************************************************************/

const SEP : &str = "/";

#[derive(Debug)]
pub struct TopicHandlerError {
    msg : String,
}

impl Display for TopicHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for TopicHandlerError {
    fn description(&self) -> &str {
        &self.msg
    }
}


impl TopicHandlerError {
    fn new(msg : &str) -> TopicHandlerError {
        TopicHandlerError {
            msg : msg.to_string(),
        }
    }
}

type Subtopics = HashMap<String, Topic>; 
type Subscribers = HashSet<String>;

impl From<PoisonError<RwLockReadGuard<'_, Subscribers>>> for TopicHandlerError {
    fn from(err : PoisonError<RwLockReadGuard<Subscribers>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("TopicHandlerError: No se pudo desbloquear contenido del Topic ({})", err))
    }
}

impl From<PoisonError<RwLockReadGuard<'_, Subtopics>>> for TopicHandlerError {
    fn from(err : PoisonError<RwLockReadGuard<Subtopics>>) -> TopicHandlerError {
        TopicHandlerError::new(&format!("TopicHandlerError: No se pudo desbloquear contenido del Topic ({})", err))
    }
}

pub struct TopicHandler<'a> {
    server : &'a Server,
    root : Topic,
}

struct Topic {
    subtopics : RwLock<Subtopics>,
    subscribers : RwLock<Subscribers>
}

impl Topic {
    fn new() -> Self {
        Topic {
            subtopics : RwLock::new(HashMap::new()),
            subscribers : RwLock::new(HashSet::new()),
        }
    }
}

impl<'a> TopicHandler<'a> {
    pub fn subscribe(&self, packet: Subscribe, client_id: &str) {
        let topic = "todo";
        todo!("todo");
    }

    pub fn publish(&self, packet: Publish) -> Result<(), TopicHandlerError> {
        let topic = "todo";

        let mut node = &self.root;
        let mut subtopics_vec = Vec::new();
        for subtopic in topic.split('/') {
            subtopics_vec.push(node.subtopics.read()?);

            match &subtopics_vec.last().unwrap().get(subtopic) {
                Some(subnode) => {
                    node = subnode;
                }
                None => {
                    return Ok(());
                }
            }
        }

        for sub in node.subscribers.read()?.iter() {
            self.server.send_publish(&packet, sub);
        }

        Ok(())
    }

    fn pub_rec(subtopics : RwLockReadGuard<Subtopics>, topic_name : &str, packet : Publish) {
        if topic_name == "" {
            //done
        }
        let next_subtopics = //
        
    }

    pub fn unsubscribe(&self, packet: Unsubscribe, client_id: &str) {

    }

    pub fn new(server: &'a Server) -> Self {
        Self {
            server,
            root : Topic::new(),
        }
    }

    fn remove_client(&self, client_id: &str) {

    }
}