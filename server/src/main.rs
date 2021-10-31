use std::{
    env,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{self, AtomicBool},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

#[allow(dead_code)]
mod config;
mod connack;
#[allow(dead_code)]
mod connect;
#[allow(dead_code)]
mod subscribe;
#[allow(dead_code)]
mod topic_handler;

#[allow(dead_code, unused_imports)]
mod server;

const SLEEP_DUR: Duration = Duration::from_secs(2);

#[allow(dead_code)]
struct Client {
    stream: TcpStream,
    connected: bool,
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for Client {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl Client {
    fn new(stream: TcpStream) -> Client {
        Client {
            stream,
            connected: false,
        }
    }
}

fn wait_for_connections(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    client_sender: Sender<Client>,
) {
    if let Err(error) = listener.set_nonblocking(true) {
        println!("Error configurando socket: {}", error.to_string());
        stop.store(true, atomic::Ordering::Relaxed);
        return;
    }

    while !stop.load(atomic::Ordering::Relaxed) {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(SLEEP_DUR);
            }
            Err(error) => {
                println!("Error aceptando conexión: {}", error.to_string());
            }
            Ok((stream, addr)) => {
                println!("Conectando con {}", addr.to_string());
                if let Err(_err) = stream.set_nonblocking(true) {
                    println!("Error estableciendo socket como no bloqueante");
                } else if let Err(_err) = client_sender.send(Client::new(stream)) {
                    println!("Error enviando cliente");
                }
            }
        }
    }
}
#[allow(dead_code)]
fn handle_packet(control_byte: [u8; 1], client: &mut Client) {
    let codigo = control_byte[0] >> 4;
    match codigo {
        1 => match connect::Connect::new(client) {
            Ok(packet) => {
                let rta = packet.response().encode();
                client.write_all(&rta).unwrap();
                println!("mandadao CONNACK con {}", packet.client_id());
            }
            Err(err) => {
                println!("Error parseando Connect packet: {}", err.to_string());
            }
        },
        2 => println!("Pendiente implementación"),
        3 => println!("Pendiente implementación"),
        4 => println!("Pendiente implementación"),
        5 => println!("Pendiente implementación"),
        6 => println!("Pendiente implementación"),
        7 => println!("Pendiente implementación"),
        8 => println!("Pendiente implementación"),
        9 => println!("Pendiente implementación"),
        10 => println!("Pendiente implementación"),
        11 => println!("Pendiente implementación"),
        12 => println!("Pendiente implementación"),
        13 => println!("Pendiente implementación"),
        14 => println!("Pendiente implementación"),
        _ => println!("Error"),
    }
}

fn wait_for_packets(stop: Arc<AtomicBool>, receiver: Receiver<Client>) {
    let mut clients = Vec::new();
    while !stop.load(atomic::Ordering::Relaxed) {
        while let Ok(client) = receiver.try_recv() {
            println!("Agregado cliente");
            clients.push(client);
        }

        for client in clients.iter_mut() {
            let mut buf = [0u8; 1];
            match client.read_exact(&mut buf) {
                Ok(_size) => {
                    handle_packet(buf, client);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    println!("Cliente se desconecto sin avisar");
                }

                Err(err) => {
                    println!(
                        "Error recibiendo bytes de stream: {}\n Kind: {:?}",
                        err.to_string(),
                        err.kind()
                    );
                }
            }
        }

        thread::sleep(SLEEP_DUR);
    }
}

fn start_server(listener: TcpListener) {
    let stop = Arc::new(AtomicBool::from(false));
    let stop_copy = stop.clone();

    let (sender, receiver) = channel();
    let handler = thread::spawn(move || {
        wait_for_connections(listener, stop_copy, sender);
    });

    wait_for_packets(stop, receiver);
    if let Err(_err) = handler.join() {
        println!("Error uniendo threads")
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Error, parámetro inválido");
        return;
    }
    match config::Config::new(&args[1]) {
        Some(config) => {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", config.port()))
                .expect("No se pudo iniciar socket");

            start_server(listener);
        }
        None => {
            println!("Error cargando configuración de {}", args[1]);
        }
    }
}
