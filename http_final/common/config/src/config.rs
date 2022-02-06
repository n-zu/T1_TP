use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub server: String,
    pub port: String,
    pub client_id: String,
    pub topic: String,
    pub user: String,
    pub password: String,
    pub period: Duration,
}

const SEP: &str = "=";

impl Config {
    pub fn new(path: &str) -> Option<Config> {
        let config_file = File::open(path).ok()?;
        Config::new_from_file(config_file)
    }

    pub fn new_from_file(config_file: impl Read) -> Option<Config> {
        let lines: Vec<String> = BufReader::new(config_file)
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let mut config: HashMap<String, String> = lines
            .iter()
            .filter(|l| !l.is_empty())
            .map(|line| {
                let (key, value) = line.trim().split_once(SEP)?;
                Some((key.to_string(), value.to_string()))
            })
            .collect::<Option<HashMap<_, _>>>()?;

        Some(Config {
            server: config.remove("server")?,
            port: config.remove("port")?,
            client_id: config.remove("client_id")?,
            topic: config.remove("topic")?,
            user: config.remove("user")?,
            password: config.remove("password")?,
            period: Duration::from_millis(config.remove("period")?.parse().ok()?),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, time::Duration};

    use crate::config::{Config, SEP};

    #[test]
    fn test_new_from_file() {
        let text = Cursor::new(
            "
            server=localhost
            port=1883
            client_id=test_client
            topic=test_topic
            user=test_user
            password=test_password
            period=1000"
                .replace("=", SEP),
        );
        let config = Config::new_from_file(text).unwrap();
        assert_eq!(config.server, "localhost");
        assert_eq!(config.port, "1883");
        assert_eq!(config.client_id, "test_client");
        assert_eq!(config.topic, "test_topic");
        assert_eq!(config.user, "test_user");
        assert_eq!(config.password, "test_password");
        assert_eq!(config.period, Duration::from_millis(1000));
    }

    #[test]
    fn test_missing_fields() {
        let text = Cursor::new(
            "
            server=localhost
            client_id=test_client
            topic=test_topic
            user=test_user
            period=1000"
                .replace("=", SEP),
        );
        let config = Config::new_from_file(text);
        assert!(config.is_none());
    }
}
