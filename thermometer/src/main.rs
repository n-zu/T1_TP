use publisher_config::PublisherConfig;
use rand::prelude::*;
use std::env;

mod publisher_config;

const MAX_TEMP: f32 = 100.0;
const MIN_TEMP: f32 = 0.0;

fn get_temperature() -> f32 {
    thread_rng().gen::<f32>() * (MAX_TEMP - MIN_TEMP) + MIN_TEMP
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = "./config.txt";
    if args.len() > 1 {
        path = &args[1];
    }
    let config = PublisherConfig::new(path).expect("Invalid config file");

    println!("{:?}", config);

    let y = get_temperature();
    println!("{}", y);
}
