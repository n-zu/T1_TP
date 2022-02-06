use crate::{ClientResult, Thermometer, ThermometerObserver};
use config::config::Config;
use mqtt_client::{Client, Message};
use packets::connect::{Connect, ConnectBuilder};
use packets::PacketResult;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use std::{env, thread};

#[doc(hidden)]
const KEEP_ALIVE: u16 = 0;
#[doc(hidden)]
const CLEAN_SESSION: bool = true;
#[doc(hidden)]
const MQTT_TIMEOUT: Duration = Duration::from_secs(5);

/// Starts the loop that allows a thermometer to publish its measure to a
/// MQTT broker
pub fn init() -> ClientResult<()> {
    let (client, config, receiver) = make_client()?;
    let stop = Arc::new(AtomicBool::new(false));
    let mut thermometer = Thermometer::new(client, config, receiver, stop.clone());
    println!("Comenzando a enviar PUBLISH");

    let handle = thread::spawn(move || {
        if let Err(e) = thermometer.publish() {
            println!("Error publicando temperaturas: {:?}", e);
        }
    });

    println!("Presione [ENTER] para detener la ejecucion del cliente\n____________\n");
    let mut buf = [0u8; 1];
    std::io::stdin().read_exact(&mut buf).unwrap_or(());
    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();
    Ok(())
}

/// Returns a valid Client and Config
#[doc(hidden)]
fn make_client() -> ClientResult<(Client<ThermometerObserver>, Config, Receiver<Message>)> {
    let config = make_config()?;

    println!("CONFIG\n{:?}\n____________\n", config);

    let connect = make_connect(&config)?;
    println!("CONNECT\n{:?}\n____________\n", connect);

    let (sender, receiver) = channel();
    let client = get_client(&config, connect, sender)?;

    println!("____________\n");

    match receiver.recv_timeout(MQTT_TIMEOUT) {
        Ok(Message::Connected(Ok(_))) => Ok((client, config, receiver)),
        Err(e) => Err(Box::new(e)),
        _ => Err("Error conectando al broker MQTT - se recibió respuesta inesperada".into()),
    }
}

/// Returns a valid Config
#[doc(hidden)]
fn make_config() -> ClientResult<Config> {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = "./config.txt";
    if args.len() > 1 {
        path = &args[1];
    }
    Config::new(path).ok_or_else(|| "Invalid config file".into())
}

/// Returns a valid CONNECT packet
#[doc(hidden)]
fn make_connect(config: &Config) -> PacketResult<Connect> {
    ConnectBuilder::new(&config.client_id, KEEP_ALIVE, CLEAN_SESSION)?
        .with_user_name(&config.user)?
        .with_password(&config.password)?
        .build()
}

/// Returns a valid Client given a config and a CONNECT packet
#[doc(hidden)]
fn get_client(
    config: &Config,
    connect: Connect,
    sender: Sender<Message>,
) -> ClientResult<Client<ThermometerObserver>> {
    Ok(Client::new(
        &format!("{}:{}", config.server, config.port),
        ThermometerObserver::new(sender),
        connect,
    )?)
}
