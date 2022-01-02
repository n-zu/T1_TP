use config::config::Config;
use mqtt_client::Client;
use observer::Observer;
use packets::{
    connect::Connect, connect::ConnectBuilder, qos::QoSLevel, subscribe::Subscribe,
    topic_filter::TopicFilter, PacketResult,
};
use std::env;
use std::io::Read;

mod observer;

const KEEP_ALIVE: u16 = 0;
const CLEAN_SESSION: bool = true;
const CONNECT_TIME: u64 = 1000;

fn get_config(name: &str, arg_num: usize) -> Config {
    let args: Vec<String> = env::args().collect();
    let mut path: &str = &format!("./{}_config.txt", name);
    if args.len() > arg_num {
        path = &args[arg_num];
    }
    Config::new(path).unwrap_or_else(|| panic!("Invalid {} config file", name))
}

fn get_connect(config: &Config) -> PacketResult<Connect> {
    ConnectBuilder::new(&config.client_id, KEEP_ALIVE, CLEAN_SESSION)?
        .with_user_name(&config.user)?
        .with_password(&config.password)?
        .build()
}

fn get_client(config: &Config, connect: Connect) -> Client<Observer> {
    Client::new(
        &format!("{}:{}", config.server, config.port),
        Observer {},
        connect,
    )
    .expect("Could not create client")
}

fn subscribe(client: &mut Client<Observer>, config: &Config) {
    let topic_filter = TopicFilter::new(String::from(&config.topic), QoSLevel::QoSLevel1).unwrap();
    let subscribe = Subscribe::new(vec![topic_filter], 2);
    println!("SUBSCRIBE\n{:?}\n____________\n", subscribe);
    client.subscribe(subscribe).expect("Could not subscribe");
}

fn main() {
    let config = get_config("mqtt", 2);
    println!("MQTT CONFIG\n{:?}\n____________\n", config);
    let _http_config = get_config("http", 1);
    println!("HTTP CONFIG\n{:?}\n____________\n", _http_config);

    let connect = get_connect(&config).expect("Could not build connect packet");
    println!("CONNECT\n{:?}\n____________\n", connect);

    let mut client = get_client(&config, connect);

    std::thread::sleep(std::time::Duration::from_millis(CONNECT_TIME));
    println!("____________\n");

    subscribe(&mut client, &config);

    println!("Presione [ENTER] para detener la ejecucion del servidor\n____________\n");
    let mut buf = [0u8; 1];
    std::io::stdin().read_exact(&mut buf).unwrap_or(());
}
