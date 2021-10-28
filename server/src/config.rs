use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
};

/// Contiene toda la informacion de configuracion
/// necesaria para iniciar el servidor
pub struct Config {
    /// Puerto al que se conecta el servidor
    port: u16,
    /// Path del archivo sobre el cual se realizara un dump
    dump_path: String,
    /// Intervalo de tiempo para el cual se realizara un dump
    dump_time: u32,
    /// Path de archivo de log
    log_path: String,
}

impl Config {
    /// Crea un struct Config a partir de la informacion
    /// que se encuentran en el archivo de configuracion
    /// 
    /// # Arguments
    /// 
    /// * `path` - Ruta del archivo que contiene la informacion.
    /// El formato del archivo es 'campo=valor' (sin espacios)
    /// en el siguiente orden: port, dump_path, dump_time, log_path
    /// 
    /// # Returns
    /// 
    /// * `Option<Config>` - Struct con la configuracion del servidor.
    /// 
    /// # Errors
    /// Si no existe el archivo o el formato es invalido, devuelve None
    pub fn new(path: &str) -> Option<Config> {
        let config_file = File::open(path).ok()?;
        Config::new_from_file(config_file)
    }

    /// Crea un struct Config a partir de un archivo de configuracion
    /// 
    /// # Arguments
    /// 
    /// * `config_file` - Archivo de configuracion
    /// 
    /// # Return
    /// 
    /// * `Option<Config>` - Struct con la configuracion del servidor
    pub fn new_from_file<F: Read>(config_file: F) -> Option<Config> {
        let mut buffered = BufReader::new(config_file);

        let port = get_value_from_line(&mut buffered, "port")?;
        let dump_path = get_value_from_line(&mut buffered, "dump_path")?;
        let dump_time = get_value_from_line(&mut buffered, "dump_time")?;
        let log_path = get_value_from_line(&mut buffered, "log_path")?;

        let port = port.parse::<u16>().ok()?;
        let dump_time = dump_time.parse::<u32>().ok()?;

        Some(Config {
            port,
            dump_path,
            dump_time,
            log_path,
        })
    }

    /// Devuelve el puerto del servidor
    pub fn port(&self) -> u16 {
        self.port
    }
    
    /// Devuelve el path del archivo sobre el cual se realizara un dump
    pub fn dump_path(&self) -> &str {
        &self.dump_path
    }

    /// Devuelve el intervalo de tiempo para el cual se realizara un dump
    pub fn dump_time(&self) -> u32 {
        self.dump_time
    }

    /// Devuelve el path de archivo de log
    pub fn log_path(&self) -> &str {
        &self.log_path
    }
}

fn get_value_from_line<F>(buffered: &mut BufReader<F>, expected_key: &str) -> Option<String>
where
    F: Read,
{
    let mut buf = String::new();

    buffered.read_line(&mut buf).ok()?;
    let key_value = buf.trim().split('=').collect::<Vec<&str>>();
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
log_path=bar.txt");

        let config = Config::new_from_file(cursor).unwrap();
        assert_eq!(config.port(), 8080);
        assert_eq!(config.dump_path(), "foo.txt");
        assert_eq!(config.dump_time(), 10);
        assert_eq!(config.log_path(), "bar.txt");
    }

    #[test]
    fn invalid_key() {
        let cursor = Cursor::new(
"invalid_key=8080
dump_path=foo.txt
dump_time=10
log_path=bar.txt");

        assert!(Config::new_from_file(cursor).is_none());
    }

    #[test]
    fn invalid_order() {
        let cursor = Cursor::new(
"port=8080
dump_time=10
dump_path=foo.txt
log_path=bar.txt");

        assert!(Config::new_from_file(cursor).is_none());
    }

    #[test]
    fn invalid_value() {
        let cursor = Cursor::new(
"port=WWWW
dump_path=foo.txt
dump_time=10
log_path=bar.txt");
            
        assert!(Config::new_from_file(cursor).is_none());
    }
}