use crate::{ClientResult, Thermometer, ThermometerObserver};
use config::config::Config;
use mqtt_client::Client;
use packets::connect::{Connect, ConnectBuilder};
use packets::PacketResult;
use std::io::Read;
use std::{env, thread};

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;
const CONNECT_TIME: u64 = 1000;

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

fn make_client() -> ClientResult<(Client<ThermometerObserver>, Config)> {
    let config = make_config();
    println!("CONFIG\n{:?}\n____________\n", config);

    let connect = make_connect(&config)?;
    println!("CONNECT\n{:?}\n____________\n", connect);

    let client = get_client(&config, connect)?;

    // FIXME: habría que somehow ponerle para que espere a que se conecte con el observer
    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));
    println!("____________\n");

    Ok((client, config))
}

fn make_config() -> Config {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = "./config.txt";
    if args.len() > 1 {
        path = &args[1];
    }
    Config::new(path).expect("Invalid config file")
}

fn make_connect(config: &Config) -> PacketResult<Connect> {
    ConnectBuilder::new(&config.client_id, KEEP_ALIVE, CLEAN_SESSION)?
        .with_user_name(&config.user)?
        .with_password(&config.password)?
        .build()
}

fn get_client(config: &Config, connect: Connect) -> ClientResult<Client<ThermometerObserver>> {
    Ok(Client::new(
        &format!("{}:{}", config.server, config.port),
        ThermometerObserver {},
        connect,
    )?)
}
