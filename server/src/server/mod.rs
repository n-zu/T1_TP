use std::{collections::HashMap, io::{self, Read, Write}, net::{SocketAddr, TcpListener, TcpStream}, sync::{
        mpsc::{sync_channel, Receiver, Sender, SyncSender},
        Arc, Mutex,
    }, thread::{self, JoinHandle}, time::Duration, vec};

use crate::{connack::Connack, connect::{self, Connect}};

const MPSC_BUF_SIZE: usize = 256;
const SLEEP_DUR: Duration = Duration::from_secs(2);

/*
trait Packet {
    fn new<T: Read>(stream: &mut T) -> Result<Box<Self>, String>;

    // Nota: Debe recibir como parametro al servidor/cliente
    fn process(&self, server: &Server);

    // Devuelve el paquete que debe enviarse como respuesta
    fn reply(&self, server: &Server) -> Option;

    fn as_bytes(self) -> Vec<u8>;

    //fn packet_type(&self) -> PacketType;
}
*/

enum Packet {
    ConnectType(Connect),
    ConnackType(Connack),
}

struct Client {
    incoming: TcpStream,
    id: Option<String>,
}

struct ClientData {
    outgoing: Mutex<TcpStream>
}

struct Server {
    client_datas: Mutex<HashMap<String, ClientData>>,
    port: u16,
    handlers: Mutex<Vec<JoinHandle<()>>>
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.incoming.read(buf)
    }
}

impl ClientData {
    fn new(outgoing: TcpStream) -> Self {
        Self {
            outgoing: Mutex::new(outgoing)
        }
    }
}

impl Server {
    fn new(port: u16) -> Arc<Self> {
        Arc::new(Self {
            client_datas: Mutex::new(HashMap::new()),
            port,
            handlers: Mutex::new(vec![])
        })
    }

    fn read_packet(&self, headers: [u8; 2], stream: &mut TcpStream) -> Packet {
        let codigo = headers[0] >> 4;

        match codigo {
            1 => match connect::Connect::new(headers, stream) {
                Ok(packet) => {
                    Packet::ConnectType(packet)
                }
                Err(_) => {
                    todo!();
                }
            },
            2 => todo!("Pendiente implementación"),
            3 => todo!("Pendiente implementación"),
            4 => todo!("Pendiente implementación"),
            5 => todo!("Pendiente implementación"),
            6 => todo!("Pendiente implementación"),
            7 => todo!("Pendiente implementación"),
            8 => todo!("Pendiente implementación"),
            9 => todo!("Pendiente implementación"),
            10 => todo!("Pendiente implementación"),
            11 => todo!("Pendiente implementación"),
            12 => todo!("Pendiente implementación"),
            13 => todo!("Pendiente implementación"),
            14 => todo!("Pendiente implementación"),
            _ => todo!("Error"),
        }
    }

    fn receive_packet(&self, stream: &mut TcpStream) -> Packet {
        let mut buf = [0u8, 2];
        match stream.read_exact(&mut buf) {
            Ok(_size) => {
                self.read_packet(buf, stream)
            }
            Err(_) => todo!()
            /*
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
            */
        }
    }

    fn handle_packet(&self, packet: Packet) {

    }
    
    fn manage_client(self: Arc<Self>, mut stream: TcpStream, addr: SocketAddr) {
        loop {
            let packet = self.receive_packet(&mut stream);
            let sv_copy = self.clone();
            let handle = thread::spawn(move || {
                sv_copy.handle_packet(packet);
            });
        }
    }

    fn connect(self: &Arc<Self>, stream: TcpStream, addr: SocketAddr) {
        println!("Conectando con {}", addr.to_string());
        if let Err(_err) = stream.set_nonblocking(true) {
            println!("Error estableciendo socket como no bloqueante");
        } else {
            let sv_copy = self.clone();
            let handle = thread::spawn(move || {
                sv_copy.manage_client(stream, addr);
            });
            self.handlers.lock().unwrap().push(handle);
        }
    }

    fn accept_client(self: &Arc<Self>, listener: &TcpListener) {
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(SLEEP_DUR);
            }
            Err(error) => {
                println!("Error aceptando conexión: {}", error.to_string());
            }
            Ok((stream, addr)) => {
                self.connect(stream, addr);
            }
        }
    }

    fn run(self: Arc<Self>) {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", 1883)).unwrap();
        loop {
            self.accept_client(&listener); 
        }
    }
}

impl Client {
    fn new(incoming: TcpStream) -> Client {
        Self {
            id: None,
            incoming,
        }
    }

    fn new_id(&mut self, id: &str) {
        self.id = Some(id.to_owned())
    }
}

fn packet_maker(headers: [u8; 2], client: &mut Client) -> Option<Packet> {
    let codigo = headers[0] >> 4;
    match codigo {
        1 => match Connect::new(headers, client) {
            Ok(packet) => {
                Some(Packet::ConnectType(packet))
            }
            Err(err) => {
                println!("Error parseando Connect packet: {}", err.to_string());
                None
            }
        },
        2 => panic!("Pendiente implementación"),
        3 => panic!("Pendiente implementación"),
        4 => panic!("Pendiente implementación"),
        5 => panic!("Pendiente implementación"),
        6 => panic!("Pendiente implementación"),
        7 => panic!("Pendiente implementación"),
        8 => panic!("Pendiente implementación"),
        9 => panic!("Pendiente implementación"),
        10 => panic!("Pendiente implementación"),
        11 => panic!("Pendiente implementación"),
        12 => panic!("Pendiente implementación"),
        13 => panic!("Pendiente implementación"),
        14 => panic!("Pendiente implementación"),
        _ => panic!("Error"),
    }
}

fn get_packet(client: &mut Client) -> Option<Packet> {
    let mut buf = [0u8, 2];
    match client.read_exact(&mut buf) {
        Ok(_size) => {
            packet_maker(buf, client)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            None
        }
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            println!("Cliente se desconecto sin avisar");
            None
        }

        Err(err) => {
            println!(
                "Error recibiendo bytes de stream: {}\n Kind: {:?}",
                err.to_string(),
                err.kind()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        net::{TcpListener, TcpStream},
        sync::{mpsc::sync_channel, Arc},
        thread,
        time::Duration,
    };

    use crate::server::{Client, MPSC_BUF_SIZE};

    use super::{Packet, Server};

    #[test]
    fn test() {
        let server = Server::new(1883);
        server.run()
    }
    /*
    #[test]
    fn test() {
        let handle = thread::spawn(|| {
            cliente();
        });

        let mut client = None;
        let (sync_sender, receiver) = sync_channel(MPSC_BUF_SIZE);
        let listener = TcpListener::bind(format!("127.0.0.1:{}", 1234)).unwrap();
        match listener.accept() {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_error) => {
                //println!("Error aceptando conexión: {}", error.to_string());
            }
            Ok((stream, _addr)) => {
                client = Some(Client::new("rust", stream, receiver));
            }
        }

        let mut server = Server::new(1234);
        {
            let server = Arc::get_mut(&mut server).unwrap();
            server.new_client(client.unwrap(), sync_sender);
        }
        server.run();
        handle.join().unwrap();
    }

    fn cliente() {
        let _stream = TcpStream::connect(format!("127.0.0.1:{}", 1234)).unwrap();
        thread::sleep(Duration::from_millis(3));
    }
    */
}
