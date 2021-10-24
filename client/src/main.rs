mod client;
mod connack;
mod connect;
use crate::connect::Connect;
use client::Client;

fn main() -> Result<(), String> {
    let mut client = Client::new("localhost:80").map_err(|err| -> String { err.to_string() })?;

    client.connect(Connect::new())?;

    println!("Conexion exitosa");
    Ok(())
}
