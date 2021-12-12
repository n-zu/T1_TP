use packets::{
    connack::Connack,
    connect::ConnectBuilder,
    traits::{MQTTDecoding, MQTTEncoding},
};
use rand::Rng;
use server::{ServerController, Server, Config};
use std::{
    io::{Cursor, Read, Write},
    net::TcpStream,
    time::Duration,
};

pub fn start_server() -> (ServerController, u32) {
    let mut port = random_port();
    let mut server = Server::new(build_config(port), 20).unwrap();
    for _ in 0..50 {
        // Intento crear el servidor bindeando a 50 puertos al azar
        if let Ok(controller) = server.run() {
            return (controller, port);
        } else {
            port = random_port();
            server = Server::new(build_config(port), 20).unwrap();
        }
    }
    panic!("No se pudo crear servidor para ejecutar el test");
}

fn random_port() -> u32 {
    // Esos números salen de esta información
    // https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers#Dynamic,_private_or_ephemeral_ports
    rand::thread_rng().gen_range(49152..65536)
}

// FIXME: por alguna razón, no escribe los logs a la ruta dada
fn build_config(port: u32) -> Config {
    let cursor = Cursor::new(format!(
        "port={}
         dump_path=tests/files/dump.txt
         dump_time=10
         log_path=tests/files/logs
         accounts_path=tests/files/test_accounts.csv
         ip=localhost",
        port
    ));

    Config::new_from_file(cursor).unwrap()
}

pub fn connect_client(
    mut builder: ConnectBuilder,
    set_auth: bool,
    port: u32,
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
