use crate::{ClientResult, Thermometer, ThermometerObserver};
use config::config::Config;
use mqtt_client::Client;
use packets::connect::{Connect, ConnectBuilder};
use packets::PacketResult;
use std::io::Read;
use std::{env, thread};

#[doc(hidden)]
const KEEP_ALIVE: u16 = 0;
#[doc(hidden)]
const CLEAN_SESSION: bool = true;
#[doc(hidden)]
const CONNECT_TIME: u64 = 1000;

/// Starts the loop that allows a thermometer to publish its measure to a
/// MQTT broker
pub fn init() {
    match make_client() {
        Ok((client, config)) => {
            let mut thermometer = Thermometer::new(client, config);
            println!("Comenzando a enviar PUBLISH");
            thread::spawn(move || {
                if let Err(e) = thermometer.publish() {
                    println!("Error publicando temperaturas: {:?}", e);
                }
            });
        }
        Err(e) => {
            println!("Error inicializando termómetro: {:?}", e);
            return;
        }
    }
    println!("Presione [ENTER] para detener la ejecucion del servidor\n____________\n");
    let mut buf = [0u8; 1];
    std::io::stdin().read_exact(&mut buf).unwrap_or(());
}

/// Returns a valid Client and Config
#[doc(hidden)]
fn make_client() -> ClientResult<(Client<ThermometerObserver>, Config)> {
    let config = make_config()?;

    println!("CONFIG\n{:?}\n____________\n", config);

    let connect = make_connect(&config)?;
    println!("CONNECT\n{:?}\n____________\n", connect);

    let client = get_client(&config, connect)?;

    // FIXME: habría que somehow ponerle para que espere a que se conecte con el observer
    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));
    println!("____________\n");

    Ok((client, config))
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
fn get_client(config: &Config, connect: Connect) -> ClientResult<Client<ThermometerObserver>> {
    Ok(Client::new(
        &format!("{}:{}", config.server, config.port),
        ThermometerObserver {},
        connect,
    )?)
}
