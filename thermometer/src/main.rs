use observer::MyObserver;
use publisher_config::PublisherConfig;
use rand::prelude::*;
use std::env;

use packets::{
    connect::Connect, connect::ConnectBuilder, publish::Publish, qos::QoSLevel, PacketResult,
};

use mqtt_client::Client;

mod observer;
mod publisher_config;

const MAX_TEMP: f32 = 100.0;
const MIN_TEMP: f32 = 0.0;

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;
const CONNECT_TIME: u64 = 1000;

fn get_config() -> PublisherConfig {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = "./config.txt";
    if args.len() > 1 {
        path = &args[1];
    }
    PublisherConfig::new(path).expect("Invalid config file")
}

fn get_connect(config: &PublisherConfig) -> PacketResult<Connect> {
    ConnectBuilder::new(&config.client_id, KEEP_ALIVE, CLEAN_SESSION)?
        .with_user_name(&config.user)?
        .with_password(&config.password)?
        .build()
}

fn get_temperature() -> f32 {
    // TODO: Replace with realistic temperature
    // ( rn it can jump from MIN to MAX between calls )
    thread_rng().gen::<f32>() * (MAX_TEMP - MIN_TEMP) + MIN_TEMP
}

fn get_temperature_publish(config: &PublisherConfig) -> Publish {
    Publish {
        packet_id: None,
        topic_name: config.topic.clone(),
        qos: QoSLevel::QoSLevel0,
        retain_flag: false,
        dup_flag: false,
        payload: format!("{}", get_temperature()),
    }
}

fn main() {
    let config = get_config();
    println!("CONFIG\n{:?}\n____________\n", config);

    let connect = get_connect(&config).expect("Could not build connect packet");
    println!("CONNECT\n{:?}\n____________\n", connect);

    let mut client = Client::new(
        &format!("{}:{}", config.server, config.port),
        MyObserver {},
        connect,
    )
    .expect("Could not create client");

    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));
    println!("____________\n");

    println!("PUBLISH");
    loop {
        let publish = get_temperature_publish(&config);
        println!("- - - - - - -\n{:?}", publish.payload);
        client.publish(publish).expect("Could not publish");
        std::thread::sleep(std::time::Duration::from_millis(config.period));
        // TODO: Shutdown
    }
}
