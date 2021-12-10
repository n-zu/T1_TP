use crate::packet_error::{ErrorKind, PacketError, PacketResult};
use crate::qos::QoSLevel;
use crate::utf8::Field;

#[derive(Debug, Clone)]
pub struct TopicFilter {
    /// Topic for a subscribe packet
    name: Field,
    qos: QoSLevel,
}

impl TopicFilter {
    /// Creates a new topic
    /// Returns PacketError if the topic name is invalid
    pub fn new(name: &str, qos: QoSLevel) -> PacketResult<TopicFilter> {
        TopicFilter::check_valid_topic_name(name)?;
        Ok(TopicFilter {
            name: Field::new_from_string(name)?,
            qos,
        })
    }

    /// Encodes the topic and returns the encoded bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.name.encode());
        bytes.push(self.qos as u8);
        bytes
    }

    // Returns the topic length
    pub fn len(&self) -> usize {
        self.name.encode().len()
        - 2 // remove 2 bytes from encoded length 
        + 1 // add 1 byte for qos
    }
    // Returns if topic is empty
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }

    /// Returns the topic name
    pub fn name(&self) -> &str {
        self.name.decode()
    }

    /// Returns the topic qos
    pub fn qos(&self) -> QoSLevel {
        self.qos
    }

    /// Sets the maximum QoS that a Topic Filter can have
    ///
    /// If the current Topic Filter has a greater QoS than the given QoS then the Topic Filter's QoS
    /// will be set to the given QoS
    pub fn set_max_qos(&mut self, max_qos: QoSLevel) {
        if (self.qos as u8) > (max_qos as u8) {
            self.qos = max_qos;
        }
    }

    /// Validates if the given topic name complies with the protocol's standard for Topic Filters
    /// MQTT-4.7
    fn check_valid_topic_name(topic_name: &str) -> PacketResult<()> {
        if topic_name.is_empty() {
            return Err(PacketError::new_kind(
                "Topic name is empty",
                ErrorKind::InvalidTopicName,
            ));
        }

        if !(topic_name.eq("#"))
            && (topic_name.matches('#').count() > 1
                || topic_name.contains('#') && !topic_name.ends_with("/#"))
        {
            return Err(PacketError::new_kind(
                "Topic name: # may only be last name in topic filter",
                ErrorKind::InvalidTopicName,
            ));
        }

        if !(topic_name.eq("+"))
            && ((topic_name.matches('+').count()
                - if topic_name.starts_with("+/") { 1 } else { 0 }
                != topic_name.matches("/+").count())
                || (topic_name.matches('+').count()
                    - if topic_name.starts_with("+/") { 1 } else { 0 }
                    != topic_name.matches("/+").count()))
        {
            return Err(PacketError::new_kind(
                &format!(
                    "Topic name: [{}]. + may not be part of topic names",
                    topic_name
                ),
                ErrorKind::InvalidTopicName,
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_topic_new() {
        let topic = TopicFilter::new("test", QoSLevel::QoSLevel0).unwrap();
        assert_eq!(topic.name(), "test");
        assert_eq!(topic.qos(), QoSLevel::QoSLevel0);
    }

    #[test]
    fn test_invalid_topic_name_empty() {
        let topic = TopicFilter::new("", QoSLevel::QoSLevel0);
        let result = topic.err().unwrap().kind();
        let expected_error = ErrorKind::InvalidTopicName;
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_topic_name_multilevel_wildcard_must_be_the_last_character_in_the_filter_on_its_own_level(
    ) {
        let topic = TopicFilter::new("hello/#/rs", QoSLevel::QoSLevel0);
        let result = topic.err().unwrap().kind();
        let expected_error = ErrorKind::InvalidTopicName;
        assert_eq!(result, expected_error);

        let topic = TopicFilter::new("hello/rs#", QoSLevel::QoSLevel0);
        let result = topic.err().unwrap().kind();
        let expected_error = ErrorKind::InvalidTopicName;
        assert_eq!(result, expected_error);
    }

    #[test]
    fn test_valid_topic_name_with_single_level_wildcard_must_occupy_an_entire_level_of_filter() {
        let topic = TopicFilter::new("hello/+", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());
    }

    #[test]
    fn test_invalid_topic_name_single_level_wildcard() {
        let topic = TopicFilter::new("hello+/rs", QoSLevel::QoSLevel0);
        assert!(topic.is_err());

        let topic = TopicFilter::new("hello/rs+", QoSLevel::QoSLevel0);
        assert!(topic.is_err());

        let topic = TopicFilter::new("hello+", QoSLevel::QoSLevel0);
        assert!(topic.is_err());

        let topic = TopicFilter::new("+hello/rs", QoSLevel::QoSLevel0);
        assert!(topic.is_err());

        let topic = TopicFilter::new("he+llo/rs", QoSLevel::QoSLevel0);
        assert!(topic.is_err());
    }

    #[test]
    fn test_valid_topic_name() {
        let topic = TopicFilter::new("hello/#", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("hello/+/world", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("hello////moto", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("hello/l/l/l/l/l/l/l", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("+", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("#", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("_/+/+//#", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("+/+/+/#", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());

        let topic = TopicFilter::new("+/+/+/+", QoSLevel::QoSLevel0);
        assert!(topic.is_ok());
    }
}
