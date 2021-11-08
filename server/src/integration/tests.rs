
use crate::integration::client_packets as cl_packets;

use std::thread::{self, JoinHandle};

use crate::{config::Config, server::Server};

use super::client_mock::{ClientMock, ClientSetMock};

fn new_thread_and_run_server() -> JoinHandle<()> {
    thread::spawn(move || {
        let config = Config {
            port: 1884,
            dump_path: "foo.txt".to_owned(),
            log_path: "bar.txt".to_owned(),
            dump_time: 10    
        };
        let server = Server::new(config, 1);
        server.run().unwrap();
    })
}

#[test]
fn test_connect_one_client() {
    new_thread_and_run_server();
    let mut client = ClientMock::new_connect_tcp("cliente");
    client.send_connect(cl_packets::ConnectBuilder::new("cliente", 15, false).unwrap().build().unwrap());
    let connack = client.receive_connack();
    assert_eq!(connack.return_code, 0);
    assert_eq!(connack.session_present, 0);
}

#[test]
fn test_connect_500_clients() {
    new_thread_and_run_server();
    let mut client_set = ClientSetMock::new();
    client_set.add_bulk(500, "client");
}