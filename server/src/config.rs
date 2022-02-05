use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    time::Duration,
};

use tracing::Level;

use crate::{
    clients_manager::simple_login::SimpleLogin,
    traits::{Config, Login},
};

/// Config struct contains information which is needed from a Server
#[derive(Debug, Clone)]
pub struct FileConfig {
    port: u16,
    dump_info: Option<(String, Duration)>,
    log_path: String,
    accounts_path: Option<String>,
    ip: String,
    log_file_level: Level,
    log_stdout_level: Level,
}

const PORT_KEY: &str = "port";
const DUMP_PATH_KEY: &str = "dump_path";
const DUMP_TIME_KEY: &str = "dump_time";
const LOG_PATH_KEY: &str = "log_path";
const ACCOUNTS_PATH_KEY: &str = "accounts_path";
const IP_KEY: &str = "ip";
const LOG_FILE_LEVEL_KEY: &str = "log_file_level";
const LOG_STDOUT_LEVEL_KEY: &str = "log_stdout_level";

const SEP: &str = "=";

impl FileConfig {
    /// Returns a Config struct based on the path file
    /// # Arguments
    ///
    /// * `path` - Path file
    /// Each line of the file must consist of `field=value`:
    /// port, dump_path, dump_time, log_path, ip
    ///
    /// # Errors
    /// If the file following the path does not have the correct format, this function returns None
    pub fn new(path: &str) -> Option<FileConfig> {
        let config_file = File::open(path).ok()?;
        FileConfig::new_from_file(config_file)
    }

    /// Returns a Config struct from a valid configuration file
    ///
    /// If config_file path does not have the correct format, this function returns None
    pub fn new_from_file(config_file: impl Read) -> Option<FileConfig> {
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

        let dump_info;
        let dump_path = config.remove(DUMP_PATH_KEY)?;
        if !dump_path.is_empty() {
            let dump_time = Duration::from_secs(config.remove(DUMP_TIME_KEY)?.parse().ok()?);
            dump_info = Some((dump_path, dump_time));
        } else {
            dump_info = None;
        }

        Some(FileConfig {
            port: config.remove(PORT_KEY)?.parse().ok()?,
            dump_info,
            log_path: config.remove(LOG_PATH_KEY)?,
            accounts_path: config.remove(ACCOUNTS_PATH_KEY),
            ip: config.remove(IP_KEY)?,
            log_file_level: config.remove(LOG_FILE_LEVEL_KEY)?.parse().ok()?,
            log_stdout_level: config.remove(LOG_STDOUT_LEVEL_KEY)?.parse().ok()?,
        })
    }

    /// Returns the file log level
    pub fn log_file_level(&self) -> Level {
        self.log_file_level
    }

    /// Returns the standard output log level
    pub fn log_stdout_level(&self) -> Level {
        self.log_stdout_level
    }
}

impl Config for FileConfig {
    fn port(&self) -> u16 {
        self.port
    }

    fn dump_info(&self) -> Option<(&str, Duration)> {
        self.dump_info
            .as_ref()
            .map(|dump_info| (dump_info.0.as_str(), dump_info.1))
    }

    fn log_path(&self) -> &str {
        &self.log_path
    }

    fn ip(&self) -> &str {
        &self.ip
    }

    fn authenticator(&self) -> Option<Box<dyn Login>> {
        let login = SimpleLogin::new(self.accounts_path.as_ref()?).ok()?;
        Some(Box::new(login))
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, time::Duration};

    use tracing::Level;

    use crate::config::FileConfig;
    use crate::traits::Config;

    #[test]
    fn test_valid_file() {
        let cursor = Cursor::new(
            "port=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt
accounts_path=
ip=localhost
log_file_level=error
log_stdout_level=info",
        );

        let config = FileConfig::new_from_file(cursor).unwrap();
        assert_eq!(config.port(), 8080);
        assert_eq!(config.dump_info().unwrap().0, "foo.txt");
        assert_eq!(config.dump_info().unwrap().1, Duration::from_secs(10));
        assert_eq!(config.log_path(), "bar.txt");
        assert!(config.authenticator().is_none());
        assert_eq!(config.ip(), "localhost");
        assert_eq!(config.log_file_level(), Level::ERROR);
        assert_eq!(config.log_stdout_level(), Level::INFO);
    }

    #[test]
    fn test_valid_file_with_whitespace() {
        let cursor = Cursor::new(
            "port=8080
    dump_path=foo.txt
  dump_time=10
log_path=bar.txt    
accounts_path=
    ip=localhost
            log_file_level=warn
        log_stdout_level=trace",
        );

        let config = FileConfig::new_from_file(cursor).unwrap();
        assert_eq!(config.port(), 8080);
        assert_eq!(config.dump_info().unwrap().0, "foo.txt");
        assert_eq!(config.dump_info().unwrap().1, Duration::from_secs(10));
        assert_eq!(config.log_path(), "bar.txt");
        assert!(config.authenticator().is_none());
        assert_eq!(config.ip(), "localhost");
        assert_eq!(config.log_file_level(), Level::WARN);
        assert_eq!(config.log_stdout_level(), Level::TRACE);
    }

    #[test]
    fn test_invalid_key() {
        let cursor = Cursor::new(
            "invalid_key=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt
accounts_path=
ip=localhost
log_file_level=warn
log_stdout_level=trace",
        );

        assert!(FileConfig::new_from_file(cursor).is_none());
    }

    #[test]
    fn test_invalid_value() {
        let cursor = Cursor::new(
            "port=WWWW
dump_path=foo.txt
dump_time=10
log_path=bar.txt
accounts_path=
ip=localhost
log_file_level=warn
log_stdout_level=trace",
        );

        assert!(FileConfig::new_from_file(cursor).is_none());
    }

    #[test]
    fn test_no_dump_info() {
        let cursor = Cursor::new(
            "port=8080
dump_path=
dump_time=
log_path=bar.txt
accounts_path=
ip=localhost
log_file_level=warn
log_stdout_level=trace",
        );

        let config = FileConfig::new_from_file(cursor).unwrap();

        assert_eq!(config.port(), 8080);
        assert!(config.dump_info().is_none());
        assert_eq!(config.log_path(), "bar.txt");
        assert!(config.authenticator().is_none());
        assert_eq!(config.ip(), "localhost");
    }
}
