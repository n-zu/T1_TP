use config::config::Config;
use logger::Logger;
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
use tracing::{debug, error, info, instrument, Level};

mod messages;
mod observer;
mod server;

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;
const CONNECT_TIME: u64 = 1000;

#[instrument(skip(arg_num))]
fn get_config(config_file: &str, arg_num: usize) -> ServerResult<Config> {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = &format!("./{}_config.txt", config_file);
    if args.len() > arg_num {
        path = &args[arg_num];
    }
    let config = Config::new(path).ok_or_else(|| "Invalid {} config file".into());
    debug!("Config cargado");
    config
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

#[instrument(skip(client, config) fields(topic_filter = %config.topic))]
fn subscribe(client: &mut Client<Observer>, config: &Config) -> ServerResult<()> {
    let topic_filter = TopicFilter::new(String::from(&config.topic), QoSLevel::QoSLevel1).unwrap();
    let subscribe = Subscribe::new(vec![topic_filter], 2);
    debug!("SUBSCRIBE");
    client.subscribe(subscribe)?;
    Ok(())
}

fn intialize_server() -> ServerResult<Client<Observer>> {
    let config = get_config("mqtt", 2)?;
    let http_config = get_config("http", 1)?;

    let connect = get_connect(&config).expect("Could not build connect packet");

    let (sender, receiver): (Sender<String>, Receiver<String>) = mpsc::channel();
    let observer = Observer::new(sender);
    let mut client = get_client(&config, connect, observer)?;

    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));

    subscribe(&mut client, &config)?;

    let server = Arc::new(Server::new(&http_config));
    server.run(receiver)?;
    Ok(client)
}

fn main() {
    let _logger = Logger::new("logs", Level::INFO, Level::DEBUG);

    match intialize_server() {
        Err(e) => error!("Error inicializando el servidor: {}", e),
        Ok(_client) => {
            info!("Presione [ENTER] para detener la ejecucion del servidor");
            let mut buf = [0u8; 1];
            std::io::stdin().read_exact(&mut buf).unwrap_or(());
        }
    }
}
