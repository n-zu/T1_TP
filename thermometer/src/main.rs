use publisher_config::PublisherConfig;
use rand::prelude::*;
use std::env;

use packets::{connect::Connect, connect::ConnectBuilder, publish::Publish, PacketResult};

mod publisher_config;

const MAX_TEMP: f32 = 100.0;
const MIN_TEMP: f32 = 0.0;

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;

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
    thread_rng().gen::<f32>() * (MAX_TEMP - MIN_TEMP) + MIN_TEMP
}

fn main() {
    let config = get_config();
    println!("{:?}", config);

    let y = get_temperature();
    println!("{}", y);

    let connect = get_connect(&config);
    println!("{:?}", connect);
}
