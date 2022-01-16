use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
};

#[derive(Debug, Clone)]
pub struct Config {
    pub server: String,
    pub port: String,
    pub client_id: String,
    pub topic: String,
    pub user: String,
    pub password: String,
    pub period: u64,
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
            period: config.remove("period")?.parse().ok()?,
        })
    }
}
