#![allow(dead_code)]

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
};

/// Config struct contains information which is needed from a Server
pub struct Config {
    port: u16,
    dump_path: String,
    dump_time: u32,
    log_path: String,
    accounts_path: String,
    ip: String,
}

const PORT_KEY: &str = "port";
const DUMP_PATH_KEY: &str = "dump_path";
const DUMP_TIME_KEY: &str = "dump_time";
const LOG_PATH_KEY: &str = "log_path";
const ACCOUNTS_PATH_KEY: &str = "accounts_path";
const IP_KEY: &str = "ip";

const SEP: &str = "=";

impl Config {
    /// Returns a Config struct based on the path file
    /// # Arguments
    ///
    /// * `path` - Path file
    /// Each line of the file must consist of 'field=value':
    /// port, dump_path, dump_time, log_path, ip
    ///
    /// # Errors
    /// If the file following the path does not have the correct format, this function returns None
    pub fn new(path: &str) -> Option<Config> {
        let config_file = File::open(path).ok()?;
        Config::new_from_file(config_file)
    }

    /// Returns a Config struct from a valid configuration file
    ///
    /// If config_file path does not have the correct format, this function returns None
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
            port: config.remove(PORT_KEY)?.parse().ok()?,
            dump_path: config.remove(DUMP_PATH_KEY)?,
            dump_time: config.remove(DUMP_TIME_KEY)?.parse().ok()?,
            log_path: config.remove(LOG_PATH_KEY)?,
            accounts_path: config.remove(ACCOUNTS_PATH_KEY)?,
            ip: config.remove(IP_KEY)?,
        })
    }

    /// Returns the port to be connected
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns path to the dump file
    pub fn dump_path(&self) -> &str {
        &self.dump_path
    }

    /// Returns the time interval between two consecutive dumps
    pub fn dump_time(&self) -> u32 {
        self.dump_time
    }

    /// Returns the path to the logs directory
    pub fn log_path(&self) -> &str {
        &self.log_path
    }

    /// Returns the path to CSV file with the accounts
    /// Format: username,password
    pub fn accounts_path(&self) -> &str {
        &self.accounts_path
    }

    /// Returns the IP address of the server
    pub fn ip(&self) -> &str {
        &self.ip
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::config::Config;

    #[test]
    fn valid_file() {
        let cursor = Cursor::new(
            "port=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt
accounts_path=baz.csv
ip=localhost",
        );

        let config = Config::new_from_file(cursor).unwrap();
        assert_eq!(config.port(), 8080);
        assert_eq!(config.dump_path(), "foo.txt");
        assert_eq!(config.dump_time(), 10);
        assert_eq!(config.log_path(), "bar.txt");
        assert_eq!(config.accounts_path(), "baz.csv");
        assert_eq!(config.ip(), "localhost");
    }

    #[test]
    fn valid_file_with_whitespace() {
        let cursor = Cursor::new(
            "port=8080
    dump_path=foo.txt
  dump_time=10
log_path=bar.txt    
accounts_path=baz.csv
    ip=localhost",
        );

        let config = Config::new_from_file(cursor).unwrap();
        assert_eq!(config.port(), 8080);
        assert_eq!(config.dump_path(), "foo.txt");
        assert_eq!(config.dump_time(), 10);
        assert_eq!(config.log_path(), "bar.txt");
        assert_eq!(config.accounts_path(), "baz.csv");
        assert_eq!(config.ip(), "localhost");
    }

    #[test]
    fn invalid_key() {
        let cursor = Cursor::new(
            "invalid_key=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt
accounts_path=baz.csv
ip=localhost",
        );

        assert!(Config::new_from_file(cursor).is_none());
    }

    #[test]
    fn invalid_value() {
        let cursor = Cursor::new(
            "port=WWWW
dump_path=foo.txt
dump_time=10
log_path=bar.txt
accounts_path=baz.csv
ip=localhost",
        );

        assert!(Config::new_from_file(cursor).is_none());
    }
}
