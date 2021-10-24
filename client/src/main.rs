mod client;
mod connack;
mod connect;

use crate::{client::Client, connect::ConnectBuilder};

fn main() -> Result<(), String> {
    let mut client = Client::new("localhost:80").map_err(|err| -> String { err.to_string() })?;

    client.connect(ConnectBuilder::new("rust", 15, false)?.build()?)?;

    println!("Conexion exitosa");
    Ok(())
}
