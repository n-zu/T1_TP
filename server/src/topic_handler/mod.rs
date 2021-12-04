#![allow(dead_code)]

use std::{
    collections::HashMap,
    fmt::Debug,
    ops::Deref,
    sync::{mpsc::Sender, RwLock},
};

pub mod topic_handler_error;

use packets::qos::QoSLevel;
use packets::{publish::Publish, subscribe::Subscribe, unsubscribe::Unsubscribe};

use self::topic_handler_error::TopicHandlerError;

type Subtopics = HashMap<String, Topic>; // key: subtopic name
type Subscribers = HashMap<String, SubscriptionData>; // key: client_id
type Subscriptions = HashMap<String, Subscribers>; // key: topic filter { key: client_id }

const SEP: &str = "/";
const MULTILEVEL_WILDCARD: &str = "#";
const SINGLELEVEL_WILDCARD: &str = "+";
const UNMATCH_WILDCARD: &str = "$";

pub struct Message {
    pub client_id: String,
    pub packet: Publish,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
struct SubscriptionData {
    qos: QoSLevel,
}

pub struct TopicHandler {
    root: Topic,
}

#[doc(hidden)]
/// Represents a Topic within a Topic Handler. A Topic node contains its subtopics, subscribers
/// multi level subscriber and single level subscriber
struct Topic {
    subtopics: RwLock<Subtopics>,
    subscribers: RwLock<Subscribers>,
    multilevel_subscribers: RwLock<Subscribers>,
    singlelevel_subscriptions: RwLock<Subscriptions>,
    retained_message: RwLock<Option<Publish>>,
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
            singlelevel_subscriptions: RwLock::new(HashMap::new()),
            retained_message: RwLock::new(None),
        }
    }
}

impl TopicHandler {
    /// Creates a new TopicHandler
    pub fn new() -> Self {
        Self { root: Topic::new() }
    }

    /// Subscribe a client id into a set of topics given a Subscribe packet
    pub fn subscribe(
        &self,
        packet: &Subscribe,
        client_id: &str,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let topics = packet.topics();
        let topics: Vec<&packets::topic_filter::TopicFilter> = topics.iter().collect();
        let mut retained = Vec::new();

        for topic_filter in topics {
            let data = SubscriptionData {
                qos: topic_filter.qos(),
            };
            retained.extend(Self::subscribe_rec(
                &self.root,
                Some(topic_filter.name()),
                client_id,
                data,
            )?);
        }

        Ok(retained)
    }

    /// Sends a Publish packet to the clients who are subscribed into a certain topic
    pub fn publish(
        &self,
        packet: &Publish,
        sender: Sender<Message>,
    ) -> Result<(), TopicHandlerError> {
        let full_topic = packet.topic_name();

        Self::publish_rec(&self.root, Some(full_topic), sender, packet, true)?;

        Ok(())
    }

    /// Unsubscribe a client_id from a set of topics given a Unsubscribe packet
    pub fn unsubscribe(
        &self,
        packet: Unsubscribe,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        for topic_name in packet.topic_filters() {
            Self::unsubscribe_rec(&self.root, Some(topic_name.name()), client_id)?;
        }
        Ok(())
    }

    /// Removes a client and all of its subscriptions
    pub fn remove_client(&self, client_id: &str) -> Result<(), TopicHandlerError> {
        Self::remove_client_rec(&self.root, client_id)?;
        Ok(())
    }

    #[doc(hidden)]
    /// remove_client recursive function
    fn remove_client_rec(node: &Topic, user_id: &str) -> Result<(), TopicHandlerError> {
        for subtopic in node.subtopics.read()?.values() {
            Self::remove_client_rec(subtopic, user_id)?;
        }

        let subs_read = node.subscribers.read()?;
        if subs_read.contains_key(user_id) {
            drop(subs_read);
            node.subscribers.write()?.remove(user_id);
        }
        node.multilevel_subscribers.write()?.remove(user_id);
        let mut singlelevel_subscriptions = node.singlelevel_subscriptions.write()?;
        Self::remove_subscriber(&mut singlelevel_subscriptions, user_id);

        // si falla no importa
        let _res = Self::clean_node(node);
        Ok(())
    }

    #[doc(hidden)]
    /// Removes a client id from given subscriptions
    fn remove_subscriber(singlelevel_subscribers: &mut Subscriptions, client_id: &str) {
        singlelevel_subscribers.retain(|_, subscribers| {
            subscribers.remove(client_id);
            !subscribers.is_empty()
        });
    }

    #[doc(hidden)]
    /// Returns true if a certain topic name matches a given topic filter
    fn topic_filter_matches(topic_filter: String, topic_name: String) -> bool {
        let name = topic_name.split(SEP).collect::<Vec<&str>>();
        let filter = topic_filter.split(SEP).collect::<Vec<&str>>();

        if filter.len() > name.len() {
            return false;
        }
        if filter.len() < name.len() && filter[filter.len() - 1] != MULTILEVEL_WILDCARD {
            return false;
        }

        for (i, filter_part) in filter.iter().enumerate() {
            if filter_part == &MULTILEVEL_WILDCARD {
                return true;
            } else if filter_part == &SINGLELEVEL_WILDCARD.to_string() {
                continue;
            } else if filter_part != &name[i].to_string() {
                return false;
            }
        }

        true
    }

    #[doc(hidden)]
    fn set_matching_subscribers(
        topic_name: Option<&str>,
        singlelevel_subscriptions: &Subscriptions,
    ) -> Vec<(String, SubscriptionData)> {
        let mut matching = Vec::new();
        if let Some(topic) = topic_name {
            for (topic_filter, subscribers) in singlelevel_subscriptions {
                if Self::topic_filter_matches(topic_filter.to_string(), topic.to_string()) {
                    matching.extend(subscribers.clone());
                }
            }
        }
        matching
    }

    // Lo tuve que hacer recursivo porque sino era un caos el tema de mantener todos los
    // locks desbloqueados, ya que no los podia dropear porque perdia las referencias internas
    #[doc(hidden)]
    /// publish recursive function
    fn publish_rec(
        node: &Topic,
        topic_name: Option<&str>,
        sender: Sender<Message>,
        packet: &Publish,
        is_root: bool,
    ) -> Result<(), TopicHandlerError> {
        let mut matching = Vec::new();
        if !(is_root && Self::starts_with_unmatch(topic_name)) {
            // Agregar los subscribers multinivel de esta hoja
            matching = Self::set_matching_subscribers(
                topic_name,
                node.singlelevel_subscriptions.read()?.deref(),
            );

            matching.extend(node.multilevel_subscribers.read()?.clone());
        }

        if topic_name.is_none() {
            matching.extend(node.subscribers.read()?.clone());
            if packet.retain_flag() {
                node.retained_message.write()?.replace(packet.clone());
            }
        }

        for (id, data) in matching {
            let mut to_be_sent = packet.clone();
            to_be_sent.set_max_qos(data.qos);
            sender.send(Message {
                client_id: id.to_string(),
                packet: to_be_sent,
            })?;
        }

        if let Some(topic) = topic_name {
            let (current, rest) = Self::split(topic);
            let subtopics = node.subtopics.read()?;
            if let Some(node) = subtopics.get(current) {
                Self::publish_rec(node, rest, sender, packet, false)?;
            } else if packet.retain_flag() {
                drop(subtopics);
                node.subtopics
                    .write()?
                    .insert(current.to_string(), Topic::new());
                let subtopics = node.subtopics.read()?;
                if let Some(node) = subtopics.get(current) {
                    Self::publish_rec(node, rest, sender, packet, false)?;
                }
            }
        }

        Ok(())
    }

    #[doc(hidden)]
    /// Adds a new client id with its data into a given topic
    fn add_single_level_subscription(
        node: &Topic,
        topic: String,
        client_id: &str,
        data: SubscriptionData,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let mut single_level_subscriptions = node.singlelevel_subscriptions.write()?;

        let single_level_subscribers = single_level_subscriptions
            .entry(topic.clone())
            .or_insert_with(HashMap::new);

        single_level_subscribers.insert(client_id.to_string(), data.clone());
        Self::get_retained_messages_rec(node, Some(&topic), data.qos)
    }

    fn add_multi_level_subscription(
        node: &Topic,
        topic: String,
        client_id: &str,
        data: SubscriptionData,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let mut multilevel_subscribers = node.multilevel_subscribers.write()?;
        multilevel_subscribers.insert(client_id.to_string(), data.clone());

        Self::get_retained_messages_rec(node, Some(&topic), data.qos)
    }

    #[doc(hidden)]
    fn handle_sub_level(
        node: &Topic,
        topic: &str,
        user_id: &str,
        sub_data: SubscriptionData,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let (current, rest) = Self::split(topic);
        match (current, rest) {
            (SINGLELEVEL_WILDCARD, _) => {
                Self::add_single_level_subscription(node, topic.to_string(), user_id, sub_data)
            }
            (MULTILEVEL_WILDCARD, _) => {
                Self::add_multi_level_subscription(node, topic.to_string(), user_id, sub_data)
            }
            _ => {
                let mut subtopics = node.subtopics.read()?;
                // Insercion de nuevo nodo
                if subtopics.get(current).is_none() {
                    drop(subtopics); //lo tengo que pedir en modo write y si esta leyendo no va a poder
                    let mut wr_subtopics = node.subtopics.write()?;
                    let subtopic = Topic::new();
                    wr_subtopics.insert(current.to_string(), subtopic);
                    drop(wr_subtopics);
                    subtopics = node.subtopics.read()?;
                }

                let subtopic = subtopics.get(current).ok_or_else(|| {
                    TopicHandlerError::new("Unexpected error, subtopic not created")
                })?;
                Self::subscribe_rec(subtopic, rest, user_id, sub_data)
            }
        }
    }

    #[doc(hidden)]
    /// Subscribe recursive function
    fn subscribe_rec(
        node: &Topic,
        topic_name: Option<&str>,
        user_id: &str,
        sub_data: SubscriptionData,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        match topic_name {
            Some(topic) => Self::handle_sub_level(node, topic, user_id, sub_data),
            None => {
                node.subscribers
                    .write()?
                    .insert(user_id.to_string(), sub_data.clone());

                Self::get_retained(node, sub_data.qos)
            }
        }
    }

    #[doc(hidden)]
    /// Removes a client id from a given topic (String) from a given Topic node
    fn remove_single_level_subscription(
        node: &Topic,
        topic: String,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        let mut single_level_subscriptions = node.singlelevel_subscriptions.write()?;
        if let Some(subscribers) = single_level_subscriptions.get_mut(&topic) {
            subscribers.remove(client_id);
            if subscribers.is_empty() {
                single_level_subscriptions.remove(&topic);
            }
        };
        Ok(())
    }

    #[doc(hidden)]
    /// unsubscribe recursive function
    fn unsubscribe_rec(
        node: &Topic,
        topic_name: Option<&str>,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        match topic_name {
            Some(topic) => {
                let (current, rest) = Self::split(topic);
                match (current, rest) {
                    (SINGLELEVEL_WILDCARD, Some(rest)) => {
                        return Self::remove_single_level_subscription(
                            node,
                            current.to_string() + SEP + rest,
                            client_id,
                        );
                    }
                    (MULTILEVEL_WILDCARD, _) => {
                        let mut multilevel_subscribers = node.multilevel_subscribers.write()?;
                        multilevel_subscribers.remove(client_id);
                        return Ok(());
                    }
                    _ => {
                        let subtopics = node.subtopics.read()?;

                        if let Some(subtopic) = subtopics.get(current) {
                            Self::unsubscribe_rec(subtopic, rest, client_id)?;
                        }
                    }
                }
            }
            None => {
                node.subscribers.write()?.remove(client_id);
            }
        }

        // si falla no importa
        let _res = Self::clean_node(node);
        Ok(())
    }

    #[doc(hidden)]
    fn clean_node(node: &Topic) -> Result<(), TopicHandlerError> {
        let mut empty_subtopics = Vec::new();

        let subtopics_read = node.subtopics.read()?;
        for (sub_topic, topic) in subtopics_read.iter() {
            if topic.subscribers.read()?.is_empty()
                && topic.subtopics.read()?.is_empty()
                && topic.multilevel_subscribers.read()?.is_empty()
                && topic.singlelevel_subscriptions.read()?.is_empty()
                && topic.retained_message.read()?.is_none()
            {
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

    #[doc(hidden)]
    /// Internal/Custom split function
    fn split(topic: &str) -> (&str, Option<&str>) {
        match topic.split_once(SEP) {
            Some((splitted, rest)) => (splitted, Some(rest)),
            None => (topic, None),
        }
    }

    #[doc(hidden)]
    /// Returns true if the given topic name starts with the unmatch wildcard
    fn starts_with_unmatch(topic_name: Option<&str>) -> bool {
        if let Some(name) = topic_name {
            return name.starts_with(UNMATCH_WILDCARD);
        }
        false
    }

    #[doc(hidden)]
    fn get_retained(node: &Topic, max_qos: QoSLevel) -> Result<Vec<Publish>, TopicHandlerError> {
        if let Some(retained) = node.retained_message.read()?.deref() {
            let mut retained = retained.clone();
            retained.set_max_qos(max_qos);
            Ok(vec![retained])
        } else {
            Ok(vec![])
        }
    }

    #[doc(hidden)]
    fn handle_retained_messages_level(
        node: &Topic,
        topic: &str,
        max_qos: QoSLevel,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        match Self::split(topic) {
            (MULTILEVEL_WILDCARD, _) => {
                let mut messages = Self::get_retained(node, max_qos)?;
                for child in node.subtopics.read()?.values() {
                    messages.extend(Self::get_retained_messages_rec(
                        child,
                        Some(MULTILEVEL_WILDCARD),
                        max_qos,
                    )?);
                }
                Ok(messages)
            }
            (SINGLELEVEL_WILDCARD, rest) => {
                let mut messages = vec![];
                for child in node.subtopics.read()?.values() {
                    messages.extend(Self::get_retained_messages_rec(child, rest, max_qos)?);
                }
                Ok(messages)
            }
            (name, rest) => {
                let mut messages = vec![];
                if let Some(child) = node.subtopics.read()?.get(name) {
                    messages.extend(Self::get_retained_messages_rec(child, rest, max_qos)?);
                }
                Ok(messages)
            }
        }
    }

    #[doc(hidden)]
    fn get_retained_messages_rec(
        node: &Topic,
        topic: Option<&str>,
        max_qos: QoSLevel,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        match topic {
            Some(topic) => Self::handle_retained_messages_level(node, topic, max_qos),
            None => Self::get_retained(node, max_qos),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec;
    use std::{collections::HashSet, sync::mpsc::channel};

    use crate::topic_handler::TopicHandler;
    use packets::publish::Publish;
    use packets::qos::QoSLevel;
    use packets::subscribe::Subscribe;
    use packets::topic_filter::TopicFilter;
    use packets::unsubscribe::Unsubscribe;

    fn build_publish(topic: &str, message: &str) -> Publish {
        Publish::new(false, QoSLevel::QoSLevel1, false, topic, message, Some(123)).unwrap()
    }

    fn build_subscribe(topic: &str) -> Subscribe {
        Subscribe::new(
            vec![TopicFilter::new(topic, QoSLevel::QoSLevel0).unwrap()],
            123,
        )
    }

    fn build_unsubscribe(topic: &str) -> Unsubscribe {
        Unsubscribe::new(
            123,
            vec![TopicFilter::new(topic, QoSLevel::QoSLevel0).unwrap()],
        )
        .unwrap()
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

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_multilevel_wildcard_one_subscribe_one_specific_publish() {
        let subscribe = build_subscribe("topic/#");
        let publish = build_publish("topic/subtopic/messages/test/001", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic/messages/test/001"
        );

        assert!(receiver.recv().is_err());
    }

    /*
    // Tests previos de una funcionalidad no obligatoria. Al final decidimos enviar una copia por suscripción.
    // [MQTT-3.3.5-1]. In addition, the Server MAY deliver further copies of the message, one for each
    // additional matching subscription and respecting the subscription’s QoS in each case.

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

           assert!(receiver.recv().is_err());
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

           assert!(receiver.recv().is_err());
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

           assert!(receiver.recv().is_err());
       }
    */
    #[test]
    fn test_deliver_copies_for_each_subscription() {
        let topic1 = TopicFilter::new("colors/#", QoSLevel::QoSLevel1).unwrap();
        let topic2 = TopicFilter::new("colors/primary/blue", QoSLevel::QoSLevel0).unwrap();
        let topic3 = TopicFilter::new("colors/+/blue", QoSLevel::QoSLevel0).unwrap();
        let subscribe = Subscribe::new(vec![topic1, topic2, topic3], 123);
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

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "colors/primary/blue");

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "colors/primary/blue");

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_unsubscribe_multilevel_stop_sending_messages_to_client() {
        let subscribe = build_subscribe("topic/auto/#");
        let unsubscribe = build_unsubscribe("topic/auto/#");
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
    fn test_topic_filter_matches() {
        // name, filter

        assert!(TopicHandler::topic_filter_matches(
            "top".to_string(),
            "top".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/sub".to_string(),
            "top/sub".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "+/sub".to_string(),
            "top/sub".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/+/leaf".to_string(),
            "top/sub/leaf".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/#".to_string(),
            "top/sub/leaf".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/+/+/green".to_string(),
            "top/sub/leaf/green".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/+/+/#".to_string(),
            "top/sub/leaf/green".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/+/+/#".to_string(),
            "top/sub/leaf/green".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/+/+/#".to_string(),
            "top/sub/leaf/green/#00FF00".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "top/+//#".to_string(),
            "top/sub//green/#00FF00".to_string()
        ));
        assert!(TopicHandler::topic_filter_matches(
            "+".to_string(),
            "fdelu".to_string()
        ));
    }

    #[test]
    fn test_singlelevel_wildcard_one_subscribe_one_publish() {
        let subscribe = build_subscribe("topic/+/leaf");
        let publish = build_publish("topic/subtopic/leaf", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic/leaf");

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_singlelevel_wildcard_complex_subscribe_one_publish() {
        let subscribe = build_subscribe("topic/+/+//leaf");
        let publish = build_publish("topic/subtopic///leaf", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic///leaf");

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_singlelevel_wildcard_complex_subscribe_3_publish() {
        let subscribe = build_subscribe("topic/+/+//leaf");
        let publish = build_publish("topic/subtopic///leaf", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender.clone()).unwrap();
        handler.publish(&publish, sender.clone()).unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic///leaf");
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic///leaf");
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic///leaf");

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_wildcards_one_subscribe_one_publish() {
        let subscribe = build_subscribe("topic/+/+//#");
        let publish = build_publish("topic/subtopic///leaf//Orangutan", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_wildcards_one_subscribe_3_publish() {
        let subscribe = build_subscribe("topic/+/+//#");
        let publish = build_publish("topic/subtopic///leaf//Orangutan", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender.clone()).unwrap();
        handler.publish(&publish, sender.clone()).unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_unsubscribe_singlelevel_stop_sending_messages_to_client() {
        let subscribe = build_subscribe("topic/+/+//leaf");
        let unsubscribe = build_unsubscribe("topic/+/+//leaf");
        let first_publish = build_publish("topic/subtopic///leaf", "unMensaje");
        let second_publish = build_publish("topic/subtopic///leaf", "otroMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&first_publish, sender.clone()).unwrap();
        handler.unsubscribe(unsubscribe, "user").unwrap();
        handler.publish(&second_publish, sender).unwrap();
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(message.packet.topic_name(), "topic/subtopic///leaf");
        // El canal se cerro sin enviar el segundo mensaje
        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_unsubscribe_wildcards_stop_sending_messages_to_client() {
        let subscribe = build_subscribe("topic/+/+//#");
        let unsubscribe = build_unsubscribe("topic/+/+//#");
        let first_publish = build_publish("topic/subtopic///leaf//Orangutan", "unMensaje");
        let second_publish = build_publish("topic/subtopic///leaf//Orangutan", "otroMensaje");
        let handler = super::TopicHandler::new();

        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&first_publish, sender.clone()).unwrap();
        handler.unsubscribe(unsubscribe, "user").unwrap();
        handler.publish(&second_publish, sender).unwrap();
        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );
        // El canal se cerro sin enviar el segundo mensaje
        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_removed_wildcards_stop_sending_messages_to_client() {
        let subscribe = build_subscribe("topic/+/+//#");
        let publish = build_publish("topic/subtopic///leaf//Orangutan", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish, sender.clone()).unwrap();
        handler.remove_client("user").unwrap();
        handler.publish(&publish, sender.clone()).unwrap();
        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );

        assert!(receiver.recv().is_err());
    }
    #[test]
    fn test_2_clients_wildcard_subscribed_one_removed() {
        let subscribe = build_subscribe("topic/+/+//#");
        let publish = build_publish("topic/subtopic///leaf//Orangutan", "unMensaje");
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user1").unwrap();
        handler.subscribe(&subscribe, "user2").unwrap();

        handler.publish(&publish, sender.clone()).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );
        let message = receiver.recv().unwrap();
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );

        handler.remove_client("user1").unwrap();

        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "user2");
        assert_eq!(
            message.packet.topic_name(),
            "topic/subtopic///leaf//Orangutan"
        );

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_topic_starting_with_dollar_sign_receive() {
        let subscribe = build_subscribe("$SYS/info"); // PANICS HERE
        let publish = build_publish("$SYS/info", "ERROR");

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "admin").unwrap();

        handler.publish(&publish, sender).unwrap();

        let message = receiver.recv().unwrap();
        assert_eq!(message.client_id, "admin");
        assert_eq!(message.packet.topic_name(), "$SYS/info");

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_topic_starting_with_dollar_sign_not_recieve_singlelevel_wildcard() {
        let subscribe = build_subscribe("+/info");
        let publish = build_publish("$SYS/info", "ERROR");

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "admin").unwrap();

        handler.publish(&publish, sender).unwrap();

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_topic_starting_with_dollar_sign_not_recieve_multilevel_wildcard() {
        let subscribe = build_subscribe("#");
        let publish = build_publish("$SYS/info", "ERROR");

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "admin").unwrap();

        handler.publish(&publish, sender).unwrap();

        assert!(receiver.recv().is_err());
    }

    #[test]
    fn test_single_wildcard_only_should_match() {
        let subscribe = build_subscribe("+");
        let publish = build_publish("hola", ":D");

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "admin").unwrap();

        handler.publish(&publish, sender).unwrap();

        assert_eq!(
            receiver.recv().unwrap().packet.payload(),
            Some(&":D".to_string())
        );
    }

    #[test]
    fn test_single_wildcard_only_should_match_multilevel() {
        let subscribe = build_subscribe("f/+");
        let publish = build_publish("f/hola", ":D");

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "admin").unwrap();

        handler.publish(&publish, sender).unwrap();

        assert_eq!(
            receiver.recv().unwrap().packet.payload(),
            Some(&":D".to_string())
        );
    }

    #[test]
    fn test_retained_messages() {
        let subscribe = build_subscribe("topic");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);

        assert_eq!(retained_messages[0].payload(), Some(&"#0000FF".to_string()));
        assert_eq!(retained_messages[0].topic_name(), "topic");
    }

    #[test]
    fn test_retained_messages_single_level_wildcard() {
        let subscribe = build_subscribe("topic/+");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic/subtopic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);

        assert_eq!(retained_messages[0].payload(), Some(&"#0000FF".to_string()));
        assert_eq!(retained_messages[0].topic_name(), "topic/subtopic");
    }

    #[test]
    fn test_retained_messages_single_level_wildcard_deeper() {
        let subscribe = build_subscribe("topic/+/subtopic");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic/a/subtopic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);

        assert_eq!(retained_messages[0].payload(), Some(&"#0000FF".to_string()));
        assert_eq!(retained_messages[0].topic_name(), "topic/a/subtopic");
    }

    #[test]
    fn test_retained_messages_multi_level_wildcard() {
        let subscribe = build_subscribe("topic/#");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic/a/subtopic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);

        assert_eq!(retained_messages[0].payload(), Some(&"#0000FF".to_string()));
        assert_eq!(retained_messages[0].topic_name(), "topic/a/subtopic");
    }
}
