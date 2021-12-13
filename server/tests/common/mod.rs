use packets::{
    connack::Connack,
    connect::ConnectBuilder,
    traits::{MQTTDecoding, MQTTEncoding},
};
use rand::Rng;
use server::{
    traits::{Login, LoginResult},
    Config, Server, ServerController,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

#[macro_export]
// Crea un HashMap<String, String> (dentro de un Some()),
// a partir de una serie de tuplas (key: &str, value: &str)
// (en realidad, usa .collect(), no crea explicitamente el HashMap)
macro_rules! usr {
    ($(($x:expr, $y:expr)),*) => {
        Some(vec![$(($x.to_string(), $y.to_string())),*].into_iter().collect())
    }
}

#[macro_export]
// Crea un vector de TopicFilters a partir de una serie
// de tuplas (topic: &str, qos: QoSLevel)
macro_rules! tpc {
    ($(($x:expr, $y:expr)),*) => {
        {
            use packets::topic_filter::TopicFilter;
            vec![$(TopicFilter::new($x, $y).unwrap()),*]
        }
    }
}

#[derive(Debug, Clone)]
struct AuthMock {
    users: HashMap<String, String>,
}

impl Login for AuthMock {
    fn login(
        &mut self,
        user_name: &str,
        password: &str,
    ) -> std::io::Result<server::traits::LoginResult> {
        if self.users.contains_key(user_name) {
            if self.users[user_name] == password {
                Ok(LoginResult::Accepted)
            } else {
                Ok(LoginResult::InvalidPassword)
            }
        } else {
            Ok(LoginResult::UsernameNotFound)
        }
    }
}

#[derive(Clone)]
struct ConfigMock {
    port: u16,
    dump_info: Option<(String, Duration)>,
    log_path: String,
    auth: Option<Box<AuthMock>>,
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

    fn ip(&self) -> &str {
        &self.ip
    }

    fn authenticator(&self) -> Option<Box<dyn Login>> {
        let authenticator = self.auth.clone()?;
        Some(authenticator)
    }
}

impl ConfigMock {
    pub fn new(
        port: u16,
        dump_info: Option<(&str, Duration)>,
        users: Option<HashMap<String, String>>,
    ) -> ConfigMock {
        ConfigMock {
            port: port,
            dump_info: dump_info.map(|(str, dur)| (str.to_string(), dur)),
            log_path: "tests/files/logs".to_string(),
            auth: users.map(|u| Box::new(AuthMock { users: u })),
            ip: "localhost".to_string(),
        }
    }
}

pub fn start_server(
    dump_info: Option<(&str, Duration)>,
    users: Option<HashMap<String, String>>,
) -> (ServerController, u16) {
    let mut port = random_port();
    let mut server = Server::new(ConfigMock::new(port, dump_info, users.clone()), 20).unwrap();
    for _ in 0..50 {
        // Intento crear el servidor bindeando a 50 puertos al azar
        if let Ok(controller) = server.run() {
            return (controller, port);
        } else {
            port = random_port();
            server = Server::new(ConfigMock::new(port, dump_info, users.clone()), 20).unwrap();
        }
    }
    panic!("No se pudo crear servidor para ejecutar el test");
}

fn random_port() -> u16 {
    // Esos números salen de esta información
    // https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers#Dynamic,_private_or_ephemeral_ports
    rand::thread_rng().gen_range(49152..=65535)
}

pub fn connect_client(builder: ConnectBuilder, port: u16, read_connack: bool) -> TcpStream {
    let mut stream = TcpStream::connect(format!("localhost:{}", port)).unwrap();

    // Los tests no deberían esperar más de 30 segundos, se configura
    // esto así no se bloquea la ejecución de los tests en un caso que falle
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();

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
