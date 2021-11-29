#![allow(dead_code)]

use std::{
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

//const OUTER_BREAK: &str = "\n";
const INNER_BREAK: &str = "=";

impl Config {
    /// Returns a Config struct based on the path file
    /// # Arguments
    ///
    /// * `path` - Path file
    /// Each line of the file must consist of 'field=value' in the following order:
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
        let mut buffered = BufReader::new(config_file);

        let port = get_value_from_line(&mut buffered, PORT_KEY)?;
        let dump_path = get_value_from_line(&mut buffered, DUMP_PATH_KEY)?;
        let dump_time = get_value_from_line(&mut buffered, DUMP_TIME_KEY)?;
        let log_path = get_value_from_line(&mut buffered, LOG_PATH_KEY)?;
        let accounts_path = get_value_from_line(&mut buffered, ACCOUNTS_PATH_KEY)?;
        let ip = get_value_from_line(&mut buffered, IP_KEY)?;

        let port = port.parse::<u16>().ok()?;
        let dump_time = dump_time.parse::<u32>().ok()?;

        Some(Config {
            port,
            dump_path,
            dump_time,
            log_path,
            accounts_path,
            ip,
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

fn get_value_from_line<F>(buffered: &mut BufReader<F>, expected_key: &str) -> Option<String>
where
    F: Read,
{
    let mut buf = String::new();

    buffered.read_line(&mut buf).ok()?;
    let key_value = buf.trim().split(INNER_BREAK).collect::<Vec<&str>>();
    if key_value.len() != 2 {
        return None;
    }
    let key = key_value[0];
    let value = key_value[1];
    if !key.eq(expected_key) {
        return None;
    }
    Some(value.to_owned())
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
    fn invalid_order() {
        let cursor = Cursor::new(
            "port=8080
dump_time=10
dump_path=foo.txt
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
