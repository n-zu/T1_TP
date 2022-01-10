use config::config::Config;
use mqtt_client::Client;
use observer::Observer;
use packets::{
    connect::Connect, connect::ConnectBuilder, qos::QoSLevel, subscribe::Subscribe,
    topic_filter::TopicFilter, PacketResult,
};
use server::{Server, ServerResult};
use std::{
    env,
    io::Read,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        Arc,
    },
};

mod macros;
mod observer;
mod server;

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;
const CONNECT_TIME: u64 = 1000;

fn get_config(name: &str, arg_num: usize) -> ServerResult<Config> {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = &format!("./{}_config.txt", name);
    if args.len() > arg_num {
        path = &args[arg_num];
    }
    Config::new(path).ok_or_else(|| "Invalid {} config file".into())
}

fn get_connect(config: &Config) -> PacketResult<Connect> {
    ConnectBuilder::new(&config.client_id, KEEP_ALIVE, CLEAN_SESSION)?
        .with_user_name(&config.user)?
        .with_password(&config.password)?
        .build()
}

fn get_client(
    config: &Config,
    connect: Connect,
    observer: Observer,
) -> ServerResult<Client<Observer>> {
    Ok(Client::new(
        &format!("{}:{}", config.server, config.port),
        observer,
        connect,
    )?)
}

fn subscribe(client: &mut Client<Observer>, config: &Config) -> ServerResult<()> {
    let topic_filter = TopicFilter::new(String::from(&config.topic), QoSLevel::QoSLevel1).unwrap();
    let subscribe = Subscribe::new(vec![topic_filter], 2);
    println!("SUBSCRIBE\n{:?}\n____________\n", subscribe);
    client.subscribe(subscribe)?;
    Ok(())
}

fn intialize_server() -> ServerResult<()> {
    let config = get_config("mqtt", 2)?;
    println!("MQTT CONFIG\n{:?}\n____________\n", config);
    let _http_config = get_config("http", 1)?;
    println!("HTTP CONFIG\n{:?}\n____________\n", _http_config);

    let connect = get_connect(&config)?;
    println!("CONNECT\n{:?}\n____________\n", connect);

    let (sender, reciever): (Sender<String>, Receiver<String>) = mpsc::channel();
    let observer = Observer::new(sender);
    let mut client = get_client(&config, connect, observer)?;

    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));
    println!("____________\n");

    subscribe(&mut client, &config)?;

    let server = Arc::new(Server::new(&_http_config));
    server.run(reciever)?;
    Ok(())
}

fn main() {
    if let Err(e) = intialize_server() {
        println!("Error inicializando el servidor: {}", e);
    }

    println!("Presione [ENTER] para detener la ejecucion del servidor\n____________\n");
    let mut buf = [0u8; 1];
    std::io::stdin().read_exact(&mut buf).unwrap_or(());
}
