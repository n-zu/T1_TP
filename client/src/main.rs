mod client;
mod connect;
mod connack;
use client::Client;
use crate::connect::Connect;

fn main() -> Result<(), String> {
    let mut client = Client::new("localhost:80").map_err(|err| -> String { err.to_string() })?;

    client.connect(Connect::new())?;

    println!("Conexion exitosa");
    Ok(())
}
