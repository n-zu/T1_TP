use packets::{
    connack::Connack,
    connect::ConnectBuilder,
    traits::{MQTTDecoding, MQTTEncoding},
};
use rand::Rng;
use server::*;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

#[derive(Clone)]
struct ConfigMock {
    port: u16,
    dump_info: Option<(String, Duration)>,
    log_path: String,
    accounts_path: String,
    ip: String,
}

impl Config for ConfigMock {
    fn port(&self) -> u16 {
        self.port
    }

    fn dump_info(&self) -> Option<(&str, Duration)> {
        self.dump_info
            .as_ref()
            .map(|(path, duration)| (path.as_str(), *duration))
    }

    fn log_path(&self) -> &str {
        &self.log_path
    }

    fn accounts_path(&self) -> &str {
        &self.accounts_path
    }

    fn ip(&self) -> &str {
        &self.ip
    }
}

pub fn start_server(dump_info: Option<(&str, Duration)>) -> (ServerController, u16) {
    let mut port = random_port();
    let mut server = Server::new(build_config(port, dump_info), 20).unwrap();
    for _ in 0..50 {
        // Intento crear el servidor bindeando a 50 puertos al azar
        if let Ok(controller) = server.run() {
            return (controller, port);
        } else {
            port = random_port();
            server = Server::new(build_config(port, dump_info), 20).unwrap();
        }
    }
    panic!("No se pudo crear servidor para ejecutar el test");
}

fn random_port() -> u16 {
    // Esos números salen de esta información
    // https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers#Dynamic,_private_or_ephemeral_ports
    rand::thread_rng().gen_range(49152..=65535)
}

// FIXME: por alguna razón, no escribe los logs a la ruta dada
fn build_config(port: u16, dump_info: Option<(&str, Duration)>) -> impl Config {
    ConfigMock {
        port: port,
        dump_info: dump_info.map(|(str, dur)| (str.to_string(), dur)),
        log_path: "tests/files/logs".to_string(),
        accounts_path: "tests/files/test_accounts.csv".to_string(),
        ip: "localhost".to_string(),
    }
}

pub fn connect_client(
    mut builder: ConnectBuilder,
    set_auth: bool,
    port: u16,
    read_connack: bool,
) -> TcpStream {
    let mut stream = TcpStream::connect(format!("localhost:{}", port)).unwrap();

    // Los tests no deberían esperar más de 30 segundos, se configura
    // esto así no se bloquea la ejecución de los tests en un caso que falle
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();

    if set_auth {
        builder = builder.user_name("user").unwrap();
        builder = builder.password("pass").unwrap();
    }
    let connect = builder.build().unwrap();
    stream.write_all(&connect.encode().unwrap()).unwrap();

    if read_connack {
        let mut control = [0u8];
        stream.read_exact(&mut control).unwrap();
        assert_eq!(control[0] >> 4, 2);
        let _ = Connack::read_from(&mut stream, control[0]).unwrap();
    }

    stream
}
