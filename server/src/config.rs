use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
};

pub struct Config {
    port: u16,
    dump_path: String,
    dump_time: u32,
    log_path: String,
}

impl Config {
    pub fn new(path: &str) -> Option<Config> {
        let config_file = File::open(path).ok()?;
        let mut buffered = BufReader::new(config_file);

        let port = get_value_from_line(&mut buffered, "port");
        let dump_path = get_value_from_line(&mut buffered, "dump_path");
        let dump_time = get_value_from_line(&mut buffered, "dump_time");
        let log_path = get_value_from_line(&mut buffered, "log_path");

        let port = port.parse::<u16>().unwrap();
        let dump_time = dump_time.parse::<u32>().unwrap();

        Some(Config {
            port,
            dump_path,
            dump_time,
            log_path,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn dump_time(&self) -> u32 {
        self.dump_time
    }
}

fn get_value_from_line<F>(buffered: &mut BufReader<F>, expected_key: &str) -> String
where
    F: Read,
{
    let mut buf = String::new();

    buffered.read_line(&mut buf).unwrap();
    let key_value = buf.trim().split('=').collect::<Vec<&str>>();
    if key_value.len() != 2 {
        panic!("Error de split de linea de Config");
    }
    let key = key_value[0];
    let value = key_value[1];
    if !key.eq(expected_key) {
        panic!("Se esperaba key {}, se encontro {}", expected_key, key);
    }
    value.to_owned()
}
/*
#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::config::Config;

    #[test]
    fn ok_test() {
        let mut cursor = Cursor::new(
"port=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt");

        let config = Config::new(&mut cursor);
        assert_eq!(config.port, 8080);
        assert_eq!(config.dump_path, "foo.txt");
        assert_eq!(config.dump_time, 10);
        assert_eq!(config.log_path, "bar.txt");
    }

    #[test]
    #[should_panic]
    fn bad_key() {
        let mut cursor = Cursor::new(
"invalid_key=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt");

        Config::new(&mut cursor);
    }
}
*/
