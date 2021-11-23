#![allow(dead_code)]

use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{mpsc::Sender, RwLock},
};

pub mod topic_handler_error;

use packets::qos::QoSLevel;
use packets::{publish::Publish, subscribe::Subscribe, unsubscribe::Unsubscribe};

use self::topic_handler_error::TopicHandlerError;

type Subtopics = HashMap<String, Topic>;
type Subscribers = HashMap<String, SubscriptionData>;

pub struct Message {
    pub client_id: String,
    pub packet: Publish,
}

#[derive(Debug, Clone)]
struct SubscriptionData {
    qos: QoSLevel,
}

#[doc(hidden)]
fn set_max_datum(subscribers: &mut Subscribers, topic: &str, datum: SubscriptionData) {
    if let Some(og_datum) = subscribers.get(&topic.to_string()) {
        if (og_datum.qos as u8) < (datum.qos as u8) {
            subscribers.insert(topic.to_string(), datum);
        }
    } else {
        subscribers.insert(topic.to_string(), datum);
    }
}
#[doc(hidden)]
fn set_max_data(subscribers: &mut Subscribers, data: Subscribers) {
    for (topic, datum) in data {
        set_max_datum(subscribers, &topic, datum);
    }
}

const SEP: &str = "/";
const EMPTY: &str = "";

pub struct TopicHandler {
    root: Topic,
}

struct Topic {
    subtopics: RwLock<Subtopics>,
    subscribers: RwLock<Subscribers>,
    multilevel_subscribers: RwLock<Subscribers>,
}

impl Debug for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Topic\n\tsubtopics: {:?}\n\tsubscribers: {:?}\n\tmultilevel_subscribers: {:?}\nsubtopic details: {:?}\n\n",
                self.subtopics.read().unwrap().keys(),
                self.subscribers.read().unwrap(),
                self.multilevel_subscribers.read().unwrap(),
                self.subtopics.read().unwrap()
            )
    }
}

impl Topic {
    fn new() -> Self {
        Topic {
            subtopics: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            multilevel_subscribers: RwLock::new(HashMap::new()),
        }
    }
}

impl TopicHandler {
    /// Subscribe a client_id into a set of topics given a Subscribe packet
    pub fn subscribe(
        &self,
        packet: &Subscribe,
        client_id: &str,
    ) -> Result<Option<Vec<Publish>>, TopicHandlerError> {
        let topics = packet.topics();
        let topics: Vec<&packets::topic::Topic> = topics.iter().collect();

        for topic_filter in topics {
            let data = SubscriptionData {
                qos: topic_filter.qos(),
            };
            subscribe_rec(&self.root, topic_filter.name(), client_id, data)?;
        }

        Ok(None)
    }

    /// Sends a Publish packet to the clients who are subscribed into a certain topic
    pub fn publish(
        &self,
        packet: &Publish,
        sender: Sender<Message>,
    ) -> Result<(), TopicHandlerError> {
        let full_topic = packet.topic_name();

        pub_rec(&self.root, full_topic, sender, packet, &mut HashMap::new())?;

        Ok(())
    }

    /// Unsubscribe a client_id from a set of topics given a Unsubscribe packet
    pub fn unsubscribe(
        &self,
        packet: Unsubscribe,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        for topic_name in packet.topic_filters() {
            unsubscribe_rec(&self.root, topic_name, client_id)?;
        }
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
fn pub_rec(
    node: &Topic,
    topic_name: &str,
    sender: Sender<Message>,
    packet: &Publish,
    multilevel_subscribers: &mut Subscribers,
) -> Result<(), TopicHandlerError> {
    // Agregar los subscribers multinivel de esta hoja
    let mlsubs = node.multilevel_subscribers.read().unwrap();
    set_max_data(multilevel_subscribers, mlsubs.clone());

    match topic_name.split_once(SEP) {
        // Aca se le puede agregar tratamiento especial para *, #, etc.
        // Uso un HashMap para no preocuparse por agregar el mismo cliente mas de una vez
        Some((topic_name, rest)) => {
            let subtopics = node.subtopics.read()?;
            if let Some(subtopic) = subtopics.get(topic_name) {
                pub_rec(subtopic, rest, sender, packet, multilevel_subscribers)?;
            }
        }
        None => {
            if topic_name != EMPTY {
                // ANTEULTIMA HOJA

                let subtopics = node.subtopics.read()?;

                if let Some(subtopic) = subtopics.get(topic_name) {
                    pub_rec(subtopic, EMPTY, sender, packet, multilevel_subscribers)?;
                } else if !multilevel_subscribers.is_empty() {
                    // Si no hay subtopics, pero hay multilevel_subscribers, entonces se envian los mensajes desde el nodo actual
                    drop(subtopics);
                    pub_rec(node, EMPTY, sender, packet, multilevel_subscribers)?;
                }
            } else {
                // ULTIIMA HOJA
                let subscribers = node.subscribers.read()?;
                set_max_data(multilevel_subscribers, subscribers.clone());
                drop(subscribers);

                for (id, data) in multilevel_subscribers.iter() {
                    let mut to_be_sent = packet.clone();
                    to_be_sent.set_max_qos(data.qos);
                    sender.send(Message {
                        client_id: id.to_string(),
                        packet: to_be_sent,
                    })?;
                }
            }
        }
    }

    Ok(())
}

fn subscribe_rec(
    node: &Topic,
    topic_name: &str,
    user_id: &str,
    sub_data: SubscriptionData,
) -> Result<(), TopicHandlerError> {
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
                subscribe_rec(subtopic, rest, user_id, sub_data)?;
            }
        }
        None => {
            if topic_name != EMPTY {
                // ANTEULTIMA HOJA
                // Wildcard MultiLevel (#)
                if topic_name == "#" {
                    let mut multilevel_subscribers = node.multilevel_subscribers.write()?;
                    multilevel_subscribers.insert(user_id.to_string(), sub_data);
                    return Ok(());
                }

                let mut subtopics = node.subtopics.read()?;
                // Insercion de nuevo nodo
                // TODO: Refactor - este codigo se repite
                if subtopics.get(topic_name).is_none() {
                    drop(subtopics); //lo tengo que pedir en modo write y si esta leyendo no va a poder
                    let mut wr_subtopics = node.subtopics.write()?;
                    let subtopic = Topic::new();
                    wr_subtopics.insert(topic_name.to_string(), subtopic);
                    drop(wr_subtopics);
                    subtopics = node.subtopics.read()?;
                }

                if let Some(subtopic) = subtopics.get(topic_name) {
                    subscribe_rec(subtopic, EMPTY, user_id, sub_data)?;
                }
            } else {
                // ULTIIMA HOJA
                node.subscribers
                    .write()?
                    .insert(user_id.to_string(), sub_data);
            }
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
            if topic_name != EMPTY {
                // ANTE ULTIMA HOJA
                let subtopics = node.subtopics.read()?;

                if let Some(subtopic) = subtopics.get(topic_name) {
                    unsubscribe_rec(subtopic, "", user_id)?;
                }
            } else {
                // ULTIMA HOJA
                node.subscribers.write()?.remove(user_id);
            }
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
    if subs_read.contains_key(user_id) {
        drop(subs_read);
        node.subscribers.write()?.remove(user_id);
    }

    // si falla no importa
    let _res = clean_node(node);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, io::Cursor, sync::mpsc::channel};

    use packets::qos::QoSLevel;
    use packets::subscribe::Subscribe;
    use packets::topic::Topic;
    use packets::traits::MQTTDecoding;
    use packets::unsubscribe::Unsubscribe;
    use packets::{publish::Publish, utf8::Field};

    fn build_publish(topic: &str, message: &str) -> Publish {
        let mut bytes = Vec::new();
        bytes.extend(Field::new_from_string(topic).unwrap().encode());
        bytes.extend([0, 1]); // identifier
        bytes.extend(Field::new_from_string(message).unwrap().encode());
        bytes.insert(0, bytes.len() as u8);
        let header = 0b00110010u8; // QoS level 1, dup false, retain false
        Publish::read_from(&mut Cursor::new(bytes), header).unwrap()
    }

    fn build_subscribe(topic: &str) -> Subscribe {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend([0, 5]); // identifier
        bytes.extend(Field::new_from_string(topic).unwrap().encode());
        bytes.push(0); // QoS level 0

        bytes.insert(0, bytes.len() as u8);
        let control_byte = 0b10000010;
        Subscribe::read_from(&mut Cursor::new(bytes), control_byte).unwrap()
    }

    fn build_unsubscribe(topic: &str) -> Unsubscribe {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend([0, 5]); // identifier
        bytes.extend(Field::new_from_string(topic).unwrap().encode());

        bytes.insert(0, bytes.len() as u8);
        let header = 0b10100010;
        Unsubscribe::read_from(&mut Cursor::new(bytes), header).unwrap()
    }

    #[test]
    fn test_one_subscribe_one_publish_single_level_topic() {
        let subscribe = build_subscribe("topic");
        let publish = build_publish("topic", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic");
    }

    #[test]
    fn test_one_subscribe_one_publish_multi_level_topic() {
        let subscribe = build_subscribe("topic/auto/casa");
        let publish = build_publish("topic/auto/casa", "unMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/auto/casa");
    }

    #[test]
    fn test_two_subscribe_one_publish_multi_level_topic() {
        let subscribe = build_subscribe("topic/auto/casa");
        let publish = build_publish("topic/auto/casa", "unMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.subscribe(&subscribe, "user2").unwrap();
        handler.publish(&publish, sender).unwrap();

        let mut pending_users: HashSet<String> = ["user".to_string(), "user2".to_string()]
            .iter()
            .cloned()
            .collect();
        for msg in receiver {
            assert!(pending_users.contains(&msg.client_id));
            pending_users.remove(&msg.client_id);
            assert_eq!(msg.packet.topic_name(), "topic/auto/casa");
        }
    }

    #[test]
    fn test_5000_subscribers() {
        let subscribe = build_subscribe("topic/auto/casa");
        let publish = build_publish("topic/auto/casa", "unMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        let mut pending_users = HashSet::new();
        for i in 0..5000 {
            let id = format!("user{}", i);
            handler.subscribe(&subscribe, &id).unwrap();
            pending_users.insert(id);
        }
        handler.publish(&publish, sender).unwrap();

        for msg in receiver {
            assert!(pending_users.contains(&msg.client_id));
            pending_users.remove(&msg.client_id);
            assert_eq!(msg.packet.topic_name(), "topic/auto/casa");
        }
    }

    #[test]
    fn test_should_reduce_qos() {
        let subscribe = build_subscribe("topic"); // Suscripción QoS 0
        let publish = build_publish("topic", "unMensaje"); // Publicación QoS 1
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.packet.qos(), QoSLevel::QoSLevel0);
    }

    #[test]
    fn test_unsubscribe_stop_sending_messages_to_client() {
        let subscribe = build_subscribe("topic/auto/casa");
        let unsubscribe = build_unsubscribe("topic/auto/casa");
        let first_publish = build_publish("topic/auto/casa", "unMensaje");
        let second_publish = build_publish("topic/auto/casa", "otroMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&first_publish, sender.clone()).unwrap();
        handler.unsubscribe(unsubscribe, "user").unwrap();
        handler.publish(&second_publish, sender).unwrap();
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/auto/casa");
        // El canal se cerro sin enviar el segundo mensaje
        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_removed_stop_sending_messages_to_client() {
        let subscribe = build_subscribe("topic/auto/casa");
        let first_publish = build_publish("topic/auto/casa", "unMensaje");
        let second_publish = build_publish("topic/auto/casa", "otroMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&first_publish, sender.clone()).unwrap();
        handler.remove_client("user").unwrap();
        handler.publish(&second_publish, sender).unwrap();
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/auto/casa");
        // El canal se cerro sin enviar el segundo mensaje
        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_unsubscribe_matches_topic_correctly() {
        let subscribe = build_subscribe("topic/auto/casa");
        let unsubscribe = build_unsubscribe("topic/no_es_auto/casa");
        let first_publish = build_publish("topic/auto/casa", "unMensaje");
        let second_publish = build_publish("topic/auto/casa", "otroMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&first_publish, sender.clone()).unwrap();
        handler.unsubscribe(unsubscribe, "user").unwrap();
        handler.publish(&second_publish, sender).unwrap();
        let first_message = receiver.recv().unwrap();
        let second_message = receiver.recv().unwrap();
        assert_eq!(first_message.client_id, "user");
        assert_eq!(first_message.packet.topic_name(), "topic/auto/casa");
        assert_eq!(second_message.client_id, "user");
        assert_eq!(second_message.packet.topic_name(), "topic/auto/casa");
    }

    #[test]
    fn test_doesnt_publish_to_siblings() {
        let subscribe = build_subscribe("topic/notsubtopic");
        let publish = build_publish("topic/subtopic", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_multilevel_wildcard_one_subscribe_one_publish() {
        let subscribe = build_subscribe("topic/#");
        let publish = build_publish("topic/subtopic", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic");
    }

    #[test]
    fn test_multilevel_wildcard_two_subscribe_one_publish() {
        let topic1 = Topic::new("colors/#", QoSLevel::QoSLevel0).unwrap();
        let topic2 = Topic::new("colors/primary/blue", QoSLevel::QoSLevel0).unwrap();
        let subscribe = Subscribe::new(vec![topic1, topic2], 123);

        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "colors/primary/blue",
            "#0000FF",
            Some(123),
        )
        .unwrap();

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "colors/primary/blue");
        assert_eq!(message.packet.payload(), Some(&"#0000FF".to_string()));

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_multilevel_wildcard_max_qos_1() {
        let topic1 = Topic::new("colors/#", QoSLevel::QoSLevel1).unwrap();
        let topic2 = Topic::new("colors/primary/blue", QoSLevel::QoSLevel0).unwrap();
        let subscribe = Subscribe::new(vec![topic1, topic2], 123);
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "colors/primary/blue",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "colors/primary/blue");
        assert_eq!(message.packet.qos(), QoSLevel::QoSLevel1);
    }

    #[test]
    fn test_multilevel_wildcard_max_qos_2() {
        let topic1 = Topic::new("colors/#", QoSLevel::QoSLevel0).unwrap();
        let topic2 = Topic::new("colors/primary/blue", QoSLevel::QoSLevel1).unwrap();
        let subscribe = Subscribe::new(vec![topic1, topic2], 123);
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            false,
            "colors/primary/blue",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "colors/primary/blue");
        assert_eq!(message.packet.qos(), QoSLevel::QoSLevel1);
    }

    #[test]
    fn test_multilevel_wildcard_max_qos_3() {
        let topic1 = Topic::new("colors/#", QoSLevel::QoSLevel1).unwrap();
        let topic2 = Topic::new("colors/primary/blue", QoSLevel::QoSLevel1).unwrap();
        let subscribe = Subscribe::new(vec![topic1, topic2], 123);
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel0,
            false,
            "colors/primary/blue",
            "#0000FF",
            None,
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "colors/primary/blue");
        assert_eq!(message.packet.qos(), QoSLevel::QoSLevel0);
    }
}
