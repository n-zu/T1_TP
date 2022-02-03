use crate::{instrument, Observer, Server, ServerGuard, ServerResult};
use config::config::Config;
use mqtt_client::Client;
use packets::connect::{Connect, ConnectBuilder};
use packets::qos::QoSLevel;
use packets::subscribe::Subscribe;
use packets::topic_filter::TopicFilter;
use packets::PacketResult;
use std::env;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use tracing::debug;

// Structures that cannot be dropped until the
// server stops
type Guards = (ServerGuard, Client<Observer>);

#[doc(hidden)]
const KEEP_ALIVE: u16 = 0;
#[doc(hidden)]
const CLEAN_SESSION: bool = true;
/// Time a thread must sleep after connecting a MQTT client to a server
const CONNECT_TIME: u64 = 1000;

/// Initialize the server with all its dependencies
pub fn initialize_server() -> ServerResult<Guards> {
    let config = make_config("mqtt", 2)?;
    let http_config = make_config("http", 1)?;

    let connect = make_connect(&config)?;

    let (sender, receiver): (Sender<String>, Receiver<String>) = mpsc::channel();
    let observer = Observer::new(sender);
    let mut client = make_client(&config, connect, observer)?;

    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));

    subscribe(&mut client, &config)?;

    let server = Arc::new(Server::new(&http_config));
    let server_guard = server.run(receiver)?;
    Ok((server_guard, client))
}

#[instrument(skip(arg_num))]
#[doc(hidden)]
fn make_config(config_file: &str, arg_num: usize) -> ServerResult<Config> {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = &format!("./{}_config.txt", config_file);
    if args.len() > arg_num {
        path = &args[arg_num];
    }
    let config = Config::new(path).ok_or("Invalid config file")?;
    debug!("Config cargado");
    Ok(config)
}

#[doc(hidden)]
fn make_connect(config: &Config) -> PacketResult<Connect> {
    ConnectBuilder::new(&config.client_id, KEEP_ALIVE, CLEAN_SESSION)?
        .with_user_name(&config.user)?
        .with_password(&config.password)?
        .build()
}

#[doc(hidden)]
fn make_client(
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

#[instrument(skip(client, config) fields(topic_filter = % config.topic))]
#[doc(hidden)]
fn subscribe(client: &mut Client<Observer>, config: &Config) -> ServerResult<()> {
    let topic_filter = TopicFilter::new(String::from(&config.topic), QoSLevel::QoSLevel1)?;
    let subscribe = Subscribe::new(vec![topic_filter], 2);
    debug!("SUBSCRIBE");
    client.subscribe(subscribe)?;
    Ok(())
}
