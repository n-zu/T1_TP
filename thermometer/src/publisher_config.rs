use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
};

#[derive(Debug)]
pub struct PublisherConfig {
    pub server: String,
    pub port: String,
    pub client_id: String,
    pub topic: String,
    pub user: String,
    pub password: String,
    pub period: u32,
}

const SEP: &str = "=";

impl PublisherConfig {
    pub fn new(path: &str) -> Option<PublisherConfig> {
        let config_file = File::open(path).ok()?;
        PublisherConfig::new_from_file(config_file)
    }

    pub fn new_from_file(config_file: impl Read) -> Option<PublisherConfig> {
        let lines: Vec<String> = BufReader::new(config_file)
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let mut config: HashMap<String, String> = lines
            .iter()
            .map(|line| {
                let (key, value) = line.trim().split_once(SEP)?;
                Some((key.to_string(), value.to_string()))
            })
            .collect::<Option<HashMap<_, _>>>()?;

        Some(PublisherConfig {
            server: config.remove("server")?,
            port: config.remove("port")?,
            client_id: config.remove("client_id")?,
            topic: config.remove("topic")?,
            user: config.remove("user")?,
            password: config.remove("password")?,
            period: config.remove("period")?.parse().ok()?,
        })
    }
}
