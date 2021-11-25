use rand::Rng;
use server::*;
use std::io::Cursor;

pub fn start_server() -> (ServerController, u32) {
    let mut port = random_port();
    let mut server = Server::new(build_config(port), 20);
    for _ in 0..50 {
        // Intento crear el servidor bindeando a 50 puertos al azar
        if let Ok(controller) = server.run() {
            return (controller, port);
        } else {
            port = random_port();
            server = Server::new(build_config(port), 20);
        }
    }
    panic!("No se pudo crear servidor para ejecutar el test");
}

fn random_port() -> u32 {
    // Esos números salen de esta información
    // https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers#Dynamic,_private_or_ephemeral_ports
    rand::thread_rng().gen_range(49152..65536)
}

fn build_config(port: u32) -> Config {
    let cursor = Cursor::new(format!(
        "port={}
         dump_path=tests/files/dump.txt
         dump_time=10
         log_path=tests/files/logs
         accounts_path=tests/files/test_accounts.csv",
        port
    ));

    Config::new_from_file(cursor).unwrap()
}
