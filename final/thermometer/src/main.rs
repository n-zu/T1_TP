use config::config::Config;
use mqtt_client::Client;
use observer::ThermometerObserver;
use packets::{
    connect::Connect, connect::ConnectBuilder, publish::Publish, qos::QoSLevel, PacketResult,
};
use rand::prelude::*;
use std::{env, error::Error, io::Read, thread};

mod observer;

const MAX_TEMP: f32 = 100.0;
const MIN_TEMP: f32 = 0.0;
const VAR_TEMP: f32 = 10.0;

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;
const CONNECT_TIME: u64 = 1000;

type ClientResult<T> = Result<T, Box<dyn Error>>;

fn get_config() -> Config {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = "./config.txt";
    if args.len() > 1 {
        path = &args[1];
    }
    Config::new(path).expect("Invalid config file")
}

fn get_connect(config: &Config) -> PacketResult<Connect> {
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

fn get_temperature(temperature: Option<f32>) -> f32 {
    match temperature {
        None => thread_rng().gen::<f32>() * (MAX_TEMP - MIN_TEMP) + MIN_TEMP,
        Some(t) => {
            let r = t + thread_rng().gen::<f32>() * VAR_TEMP * 2.0 - VAR_TEMP;
            match r {
                t if t > MAX_TEMP => MAX_TEMP,
                t if t < MIN_TEMP => MIN_TEMP,
                _ => r,
            }
        }
    }
}

fn get_temperature_publish(config: &Config, temperature: f32) -> PacketResult<Publish> {
    Publish::new(
        false,
        QoSLevel::QoSLevel0,
        false,
        &config.topic,
        &temperature.to_string(),
        None,
    )
}

fn publish_temperature(
    client: &mut Client<ThermometerObserver>,
    config: &Config,
) -> ClientResult<()> {
    let mut temperature = get_temperature(None);

    loop {
        temperature = get_temperature(Some(temperature));
        let publish = get_temperature_publish(config, temperature)?;
        println!("- - - - - - -\n{:?}", publish.payload());
        client.publish(publish).expect("Could not publish");
        std::thread::sleep(std::time::Duration::from_millis(config.period));
    }
    //Ok(())
}

fn initialize_thermometer() -> ClientResult<(Client<ThermometerObserver>, Config)> {
    let config = get_config();
    println!("CONFIG\n{:?}\n____________\n", config);

    let connect = get_connect(&config)?;
    println!("CONNECT\n{:?}\n____________\n", connect);

    let client = get_client(&config, connect)?;

    // FIXME: habría que somehow ponerle para que espere a que se conecte con el observer
    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));
    println!("____________\n");

    Ok((client, config))
}

fn main() {
    match initialize_thermometer() {
        Ok((mut client, config)) => {
            println!("PUBLISH");
            thread::spawn(move || {
                if let Err(e) = publish_temperature(&mut client, &config) {
                    println!("Error recibiendo temperaturas: {:?}", e);
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
