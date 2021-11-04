mod client;
mod connack;
mod connect;
mod disconnect;
mod publish;
mod subscribe;

use crate::{client::Client, connect::ConnectBuilder};

fn main() -> Result<(), String> {
    let mut client = Client::new("127.0.0.1:1883").map_err(|err| -> String { err.to_string() })?;

    client.connect(ConnectBuilder::new("rust", 15, false)?.build()?)?;

    println!("Conexion exitosa");
    Ok(())
}
