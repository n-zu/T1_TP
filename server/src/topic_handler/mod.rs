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

type Subscription = (String, SubscriptionData); // client_id, data
type Subtopics = HashMap<String, Topic>; // key: subtopic name
type Subscribers = HashMap<String, SubscriptionData>; // key: client_id
type Subscriptions = HashMap<String, Subscribers>; // key: topic filter { key: client_id }

const SEP: &str = "/";
const MULTI_LEVEL_WILDCARD: &str = "#";
const SINGLE_LEVEL_WILDCARD: &str = "+";
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
    /// Returns a new Topic struct
    fn new() -> Self {
        Topic {
            subtopics: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            multilevel_subscribers: RwLock::new(HashMap::new()),
            singlelevel_subscriptions: RwLock::new(HashMap::new()),
            retained_message: RwLock::new(None),
        }
    }

    /// Sends a Publish packet to the clients who are subscribed into a certain topic
    fn publish(
        &self,
        topic_name: Option<&str>,
        sender: Sender<Message>,
        packet: &Publish,
        is_root: bool,
    ) -> Result<(), TopicHandlerError> {
        let matching = self.current_matching_subs(topic_name, is_root)?;
        let mut packet_no_retain = packet.clone();
        packet_no_retain.set_retain_flag(false);
        TopicHandler::send_publish(&sender, &packet_no_retain, &matching)?;
        match topic_name {
            Some(topic) => {
                let (current, rest) = Self::split(topic);
                let mut subtopics = self.subtopics.read()?;
                if !subtopics.contains_key(current)
                    && packet.retain_flag()
                    && !packet.payload().is_empty()
                {
                    drop(subtopics);
                    self.subtopics
                        .write()?
                        .insert(current.to_string(), Topic::new());
                    subtopics = self.subtopics.read()?;
                }
                if let Some(subtopic) = subtopics.get(current) {
                    subtopic.publish(rest, sender, packet, false)?;
                    if subtopic.is_empty()? {
                        drop(subtopics);
                        self.subtopics.write()?.remove(current);
                    }
                }
            }
            None => {
                self.update_retained_message(packet)?;
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    /// Subscribe a client id into a topic
    fn subscribe(
        &self,
        topic_name: Option<&str>,
        client_id: &str,
        sub_data: SubscriptionData,
        is_root: bool,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        match topic_name {
            Some(topic) => self.handle_sub_level(topic, client_id, sub_data, is_root),
            None => {
                self.subscribers
                    .write()?
                    .insert(client_id.to_string(), sub_data.clone());

                self.get_retained(sub_data.qos)
            }
        }
    }

    #[doc(hidden)]
    /// Gets the retained message of a given topic in a Vec
    /// The Vec will be empty if there is no retained message
    fn get_retained(&self, max_qos: QoSLevel) -> Result<Vec<Publish>, TopicHandlerError> {
        if let Some(retained) = self.retained_message.read()?.deref() {
            let mut retained = retained.clone();
            retained.set_max_qos(max_qos);
            Ok(vec![retained])
        } else {
            Ok(vec![])
        }
    }

    #[doc(hidden)]
    /// Helper function for subscribe_rec() that walks through the current tree level according to
    /// the remaining part of the topic name currently being processed
    fn handle_sub_level(
        &self,
        topic: &str,
        user_id: &str,
        sub_data: SubscriptionData,
        is_root: bool,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let (current, rest) = Self::split(topic);
        match (current, rest) {
            (SINGLE_LEVEL_WILDCARD, _) => {
                self.add_single_level_subscription(topic, user_id, sub_data, is_root)
            }
            (MULTI_LEVEL_WILDCARD, _) => {
                self.add_multi_level_subscription(topic, user_id, sub_data, is_root)
            }
            _ => {
                let mut subtopics = self.subtopics.read()?;
                if subtopics.get(current).is_none() {
                    drop(subtopics);
                    self.subtopics
                        .write()?
                        .insert(current.to_string(), Topic::new());
                    subtopics = self.subtopics.read()?;
                }

                let subtopic = subtopics.get(current).ok_or_else(|| {
                    TopicHandlerError::new("Unexpected error, subtopic not created")
                })?;
                subtopic.subscribe(rest, user_id, sub_data, false)
            }
        }
    }

    #[doc(hidden)]
    /// Adds a new client id with its data into a given topic's single level subscriptions
    /// Returns the matching retained messages
    fn add_single_level_subscription(
        &self,
        topic: &str,
        client_id: &str,
        data: SubscriptionData,
        is_root: bool,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let mut single_level_subscriptions = self.singlelevel_subscriptions.write()?;

        let single_level_subscribers = single_level_subscriptions
            .entry(topic.to_string())
            .or_insert_with(HashMap::new);

        single_level_subscribers.insert(client_id.to_string(), data.clone());

        self.get_retained_messages_rec(Some(topic), data.qos, is_root)
    }

    #[doc(hidden)]
    /// Adds a new client id with its data into a given topic's multi level subscriptions
    /// Returns the matching retained messages
    fn add_multi_level_subscription(
        &self,
        topic: &str,
        client_id: &str,
        data: SubscriptionData,
        is_root: bool,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        let mut multilevel_subscribers = self.multilevel_subscribers.write()?;
        multilevel_subscribers.insert(client_id.to_string(), data.clone());
        self.get_retained_messages_rec(Some(topic), data.qos, is_root)
    }

    #[doc(hidden)]
    /// recursively gets all the matching retained messages of a given topic
    fn get_retained_messages_rec(
        &self,
        topic: Option<&str>,
        max_qos: QoSLevel,
        unmatch: bool,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        match topic {
            Some(topic) => self.handle_retained_messages_level(topic, max_qos, unmatch),
            None => self.get_retained(max_qos),
        }
    }

    #[doc(hidden)]
    /// Helper function for get_retained_messages_rec() that walks through the current
    /// tree level according to the remaining part of the topic name currently being processed
    fn handle_retained_messages_level(
        &self,
        topic: &str,
        max_qos: QoSLevel,
        unmatch: bool,
    ) -> Result<Vec<Publish>, TopicHandlerError> {
        match Self::split(topic) {
            (MULTI_LEVEL_WILDCARD, _) => {
                let mut messages = self.get_retained(max_qos)?;
                for (name, child) in self.subtopics.read()?.deref() {
                    if !(unmatch && name.starts_with(UNMATCH_WILDCARD)) {
                        messages.extend(child.get_retained_messages_rec(
                            Some(MULTI_LEVEL_WILDCARD),
                            max_qos,
                            false,
                        )?);
                    }
                }
                Ok(messages)
            }
            (SINGLE_LEVEL_WILDCARD, rest) => {
                let mut messages = vec![];
                for (name, child) in self.subtopics.read()?.deref() {
                    if !(unmatch && name.starts_with(UNMATCH_WILDCARD)) {
                        messages.extend(child.get_retained_messages_rec(rest, max_qos, false)?);
                    }
                }
                Ok(messages)
            }
            (name, rest) => {
                let mut messages = vec![];
                if let Some(child) = self.subtopics.read()?.get(name) {
                    messages.extend(child.get_retained_messages_rec(rest, max_qos, false)?);
                }
                Ok(messages)
            }
        }
    }

    #[doc(hidden)]
    /// If the packet is a retained message, it either updates the retained message of the topic or
    /// it removes its retained message if it the packet has a zero-length payload ([MQTT-3.3.1-11])
    fn update_retained_message(&self, packet: &Publish) -> Result<(), TopicHandlerError> {
        if packet.retain_flag() {
            let mut retained = self.retained_message.write()?;
            if packet.payload().is_empty() {
                *retained = None;
            } else {
                *retained = Some(packet.clone());
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    /// Gets the matching subscriptions of the given topic for the given topic name
    fn current_matching_subs(
        &self,
        topic_name: Option<&str>,
        is_root: bool,
    ) -> Result<Vec<Subscription>, TopicHandlerError> {
        let mut matching = Vec::new();
        if !(is_root && Self::starts_with_unmatch(topic_name)) {
            matching.extend(Self::current_matching_single_level_subs(
                topic_name,
                self.singlelevel_subscriptions.read()?.deref(),
            ));

            matching.extend(self.multilevel_subscribers.read()?.clone());
        }

        if topic_name.is_none() {
            matching.extend(self.subscribers.read()?.clone());
        }

        Ok(matching)
    }

    #[doc(hidden)]
    /// Unsubscribe a given client id from a given topic name
    fn unsubscribe(
        &self,
        topic_name: Option<&str>,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        match topic_name {
            Some(topic) => {
                let (current, rest) = Self::split(topic);
                match (current, rest) {
                    (SINGLE_LEVEL_WILDCARD, Some(rest)) => {
                        return self.remove_single_level_subscription(
                            &(current.to_string() + SEP + rest),
                            client_id,
                        );
                    }
                    (MULTI_LEVEL_WILDCARD, _) => {
                        let mut multilevel_subscribers = self.multilevel_subscribers.write()?;
                        multilevel_subscribers.remove(client_id);
                        return Ok(());
                    }
                    _ => {
                        let subtopics = self.subtopics.read()?;

                        if let Some(subtopic) = subtopics.get(current) {
                            subtopic.unsubscribe(rest, client_id)?;
                            if subtopic.is_empty()? {
                                drop(subtopics);
                                self.subtopics.write()?.remove(current);
                            }
                        }
                    }
                }
            }
            None => {
                self.subscribers.write()?.remove(client_id);
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    /// Removes a client id from a given topic name
    fn remove_single_level_subscription(
        &self,
        topic_name: &str,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        let mut single_level_subscriptions = self.singlelevel_subscriptions.write()?;
        if let Some(subscribers) = single_level_subscriptions.get_mut(topic_name) {
            subscribers.remove(client_id);
            if subscribers.is_empty() {
                single_level_subscriptions.remove(topic_name);
            }
        };
        Ok(())
    }

    #[doc(hidden)]
    /// Removes all the information from a given client_id
    fn remove_client(&self, client_id: &str) -> Result<(), TopicHandlerError> {
        let mut to_be_cleaned = Vec::new();
        for (name, subtopic) in self.subtopics.read()?.deref() {
            subtopic.remove_client(client_id)?;
            if subtopic.is_empty()? {
                to_be_cleaned.push(name.to_string());
            }
        }
        if !to_be_cleaned.is_empty() {
            let mut subtopics = self.subtopics.write()?;
            for name in to_be_cleaned {
                subtopics.remove(&name);
            }
        }
        self.remove_subscriber(client_id)?;
        Ok(())
    }

    #[doc(hidden)]
    /// Removes a client id's subscriptions
    fn remove_subscriber(&self, client_id: &str) -> Result<(), TopicHandlerError> {
        let subs_read = self.subscribers.read()?;
        if subs_read.contains_key(client_id) {
            drop(subs_read);
            self.subscribers.write()?.remove(client_id);
        }
        self.multilevel_subscribers.write()?.remove(client_id);
        self.singlelevel_subscriptions
            .write()?
            .retain(|_, subscribers| {
                subscribers.remove(client_id);
                !subscribers.is_empty()
            });
        Ok(())
    }

    #[doc(hidden)]
    /// Returns true if all the subtopics, subscribers, subscription and retained messages
    /// are empty
    fn is_empty(&self) -> Result<bool, TopicHandlerError> {
        Ok(self.subtopics.read()?.is_empty()
            && self.subscribers.read()?.is_empty()
            && self.multilevel_subscribers.read()?.is_empty()
            && self.singlelevel_subscriptions.read()?.is_empty()
            && self.retained_message.read()?.is_none())
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
    /// Returns all the matching subscriptions for a given topic name
    fn current_matching_single_level_subs(
        topic_name: Option<&str>,
        single_level_subscriptions: &Subscriptions,
    ) -> Vec<Subscription> {
        let mut matching = Vec::new();

        if let Some(topic) = topic_name {
            for (topic_filter, subscribers) in single_level_subscriptions {
                if Self::topic_filter_matches(topic_filter, topic) {
                    matching.extend(subscribers.clone());
                }
            }
        }
        matching
    }

    #[doc(hidden)]
    /// Returns true if a certain topic name matches a given topic filter
    fn topic_filter_matches(topic_filter: &str, topic_name: &str) -> bool {
        let name = topic_name.split(SEP).collect::<Vec<&str>>();
        let filter = topic_filter.split(SEP).collect::<Vec<&str>>();

        if filter.len() > name.len() {
            return false;
        }
        if filter.len() < name.len() && filter[filter.len() - 1] != MULTI_LEVEL_WILDCARD {
            return false;
        }
        for (i, filter_part) in filter.iter().enumerate() {
            if filter_part == &MULTI_LEVEL_WILDCARD {
                return true;
            } else if filter_part == &SINGLE_LEVEL_WILDCARD.to_string() {
                continue;
            } else if filter_part != &name[i].to_string() {
                return false;
            }
        }
        true
    }

    #[doc(hidden)]
    /// Internal/Custom split function
    fn split(topic: &str) -> (&str, Option<&str>) {
        match topic.split_once(SEP) {
            Some((splitted, rest)) => (splitted, Some(rest)),
            None => (topic, None),
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
            retained.extend(self.root.subscribe(
                Some(topic_filter.name()),
                client_id,
                data,
                true,
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
        self.root.publish(Some(full_topic), sender, packet, true)?;
        Ok(())
    }

    /// Unsubscribe a client_id from a set of topics given a Unsubscribe packet
    pub fn unsubscribe(
        &self,
        packet: Unsubscribe,
        client_id: &str,
    ) -> Result<(), TopicHandlerError> {
        for topic_name in packet.topic_filters() {
            self.root.unsubscribe(Some(topic_name.name()), client_id)?;
        }
        Ok(())
    }

    /// Removes a client and all of its subscriptions
    pub fn remove_client(&self, client_id: &str) -> Result<(), TopicHandlerError> {
        self.root.remove_client(client_id)?;
        Ok(())
    }

    #[doc(hidden)]
    /// Sends a publish packet to the given subscribers, adjusting the QoS if needed
    fn send_publish(
        sender: &Sender<Message>,
        packet: &Publish,
        subscribers: &[Subscription],
    ) -> Result<(), TopicHandlerError> {
        for (id, data) in subscribers {
            let mut to_be_sent = packet.clone();
            to_be_sent.set_max_qos(data.qos);
            sender.send(Message {
                client_id: id.to_string(),
                packet: to_be_sent,
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;
    use std::{collections::HashSet, sync::mpsc::channel};

    use crate::topic_handler::Topic;
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

        assert!(Topic::topic_filter_matches("top", "top"));
        assert!(Topic::topic_filter_matches("top/sub", "top/sub"));
        assert!(Topic::topic_filter_matches("+/sub", "top/sub"));
        assert!(Topic::topic_filter_matches("top/+/leaf", "top/sub/leaf"));
        assert!(Topic::topic_filter_matches("top/#", "top/sub/leaf"));
        assert!(Topic::topic_filter_matches(
            "top/+/+/green",
            "top/sub/leaf/green"
        ));
        assert!(Topic::topic_filter_matches(
            "top/+/+/#",
            "top/sub/leaf/green"
        ));
        assert!(Topic::topic_filter_matches(
            "top/+/+/#",
            "top/sub/leaf/green"
        ));
        assert!(Topic::topic_filter_matches(
            "top/+/+/#",
            "top/sub/leaf/green/#00FF00"
        ));
        assert!(Topic::topic_filter_matches(
            "top/+//#",
            "top/sub//green/#00FF00"
        ));
        assert!(Topic::topic_filter_matches("+", "fdelu"));
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
        let subscribe = build_subscribe("$SYS/info");
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

        assert_eq!(receiver.recv().unwrap().packet.payload(), ":D");
    }

    #[test]
    fn test_single_wildcard_only_should_match_multilevel() {
        let subscribe = build_subscribe("f/+");
        let publish = build_publish("f/hola", ":D");

        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "admin").unwrap();

        handler.publish(&publish, sender).unwrap();

        assert_eq!(receiver.recv().unwrap().packet.payload(), ":D");
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
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
        assert_eq!(retained_messages[0].topic_name(), "topic");
    }

    #[test]
    fn test_retained_messages_not_on_siblings() {
        let subscribe = build_subscribe("other_topic");
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

        assert_eq!(retained_messages.len(), 0);
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
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
        assert_eq!(retained_messages[0].topic_name(), "topic/subtopic");
    }

    #[test]
    fn test_retained_messages_single_level_wildcard_negative() {
        let subscribe = build_subscribe("topic/+");
        let publish_1 = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic/sub/subsub",
            "#0000FF",
            Some(123),
        )
        .unwrap();

        let publish_2 = Publish::new(
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

        handler.publish(&publish_1, sender.clone()).unwrap();
        handler.publish(&publish_2, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 0);
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
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
        assert_eq!(retained_messages[0].topic_name(), "topic/a/subtopic");
    }

    #[test]
    fn test_retained_messages_single_level_wildcard_multiple() {
        let subscribe = build_subscribe("topic/+//+/subtopic");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic/a//b/subtopic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
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
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
        assert_eq!(retained_messages[0].topic_name(), "topic/a/subtopic");
    }

    #[test]
    fn test_retained_messages_combined_wildcards() {
        let subscribe = build_subscribe("topic/+//+/subtopic/#");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic/a//b/subtopic/cat/white",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
        assert_eq!(
            retained_messages[0].topic_name(),
            "topic/a//b/subtopic/cat/white"
        );
    }

    #[test]
    fn test_retained_messages_wildcards_topic_starting_with_dollar_sign() {
        let subscribe = build_subscribe("#");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "$SYS",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 0);
    }

    #[test]
    fn test_retained_messages_matches_dollar_sign() {
        let subscribe = build_subscribe("$SYS/logs");
        let publish = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "$SYS/logs",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "#0000FF");
    }

    #[test]
    fn test_retained_messages_one_matches_one_starts_with_dollar_sign() {
        let subscribe = build_subscribe("#");
        let publish1 = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "SYS/logs",
            "spam",
            Some(123),
        )
        .unwrap();
        let publish2 = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "$SYS/logs",
            "fdelu",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish1, sender.clone()).unwrap();
        handler.publish(&publish2, sender).unwrap();

        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 1);
        assert!(retained_messages[0].retain_flag());
        assert_eq!(retained_messages[0].payload(), "spam");
    }

    #[test]
    fn test_retained_messages_zero_length_should_not_be_stored() {
        let subscribe = build_subscribe("topic");
        let publish =
            Publish::new(false, QoSLevel::QoSLevel1, true, "topic", "", Some(123)).unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 0);
    }

    #[test]
    fn test_retained_messages_zero_length_should_remove_retained_message() {
        let subscribe = build_subscribe("topic");
        let publish_1 = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let publish_2 =
            Publish::new(false, QoSLevel::QoSLevel1, true, "topic", "", Some(123)).unwrap();
        let handler = super::TopicHandler::new();
        let (sender, _r) = channel();

        handler.publish(&publish_1, sender.clone()).unwrap();
        handler.publish(&publish_2, sender).unwrap();
        let retained_messages = handler.subscribe(&subscribe, "user").unwrap();

        assert_eq!(retained_messages.len(), 0);
    }

    #[test]
    fn test_retained_message_should_not_be_retained_when_first_sent() {
        let subscribe = build_subscribe("topic");
        let publish_1 = Publish::new(
            false,
            QoSLevel::QoSLevel1,
            true,
            "topic",
            "#0000FF",
            Some(123),
        )
        .unwrap();
        let handler = super::TopicHandler::new();
        let (sender, receiver) = channel();

        handler.subscribe(&subscribe, "user").unwrap();
        handler.publish(&publish_1, sender).unwrap();

        let msg = receiver.recv().unwrap();
        assert_eq!(msg.client_id, "user");
        assert!(!msg.packet.retain_flag());
    }
}
