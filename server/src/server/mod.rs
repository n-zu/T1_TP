use std::{collections::HashMap, io::{self, Read, Write}, net::{TcpListener, TcpStream}, sync::{
        mpsc::{sync_channel, Receiver, Sender, SyncSender},
        Arc, Mutex,
    }, thread, time::Duration};

use crate::{connack::Connack, connect::Connect};

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
    stream: TcpStream,
    pending: Option<Receiver<Packet>>,
    id: Option<String>,
}

struct ClientData {
    sender: Mutex<SyncSender<Packet>>
}

struct Queue {
    head: Mutex<Receiver<Client>>,
    tail: SyncSender<Client>,
}

struct Topic {}

struct Petition {
    client: Client,
    packet: Packet
}

struct Petitions {
    sender: SyncSender<Petition>,
    receiver: Mutex<Receiver<Petition>>,
}
struct Server {
    queue: Queue,
    petitions: Petitions,
    client_datas: HashMap<String, ClientData>,
    port: u16,
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Petitions {
    fn new() -> Self {
        let (sync_sender, receiver) = sync_channel(MPSC_BUF_SIZE);
        Self {
            sender: sync_sender,
            receiver: Mutex::new(receiver),
        }
    }
}

impl Queue {
    fn new() -> Self {
        let (sync_sender, receiver) = sync_channel(MPSC_BUF_SIZE);
        Self {
            head: Mutex::new(receiver),
            tail: sync_sender,
        }
    }

    fn enqueue(&self, client: Client) {
        self.tail.send(client).unwrap();
    }

    fn dequeue(&self) -> Option<Client> {
        if let Ok(client) = self.head.lock().unwrap().try_recv() {
            return Some(client);
        }
        None
    }
}

fn listen_for_clients(queue_tail: SyncSender<Client>) {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", 1883)).unwrap();
    loop {
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
                } else if let Err(_err) = queue_tail.send(Client::new(stream)) {
                    println!("Error enviando cliente");
                }
            }
        }
    }
}

impl ClientData {
    fn new(sender: SyncSender<Packet>) -> Self {
        Self {
            sender: Mutex::new(sender)
        }
    }
}

fn handle_petitions(server: &mut Server) {
    while let Ok(mut petition) = server.petitions.receiver.lock().unwrap().try_recv() {
        println!("Manejando petition particular");
        match petition.packet {
            Packet::ConnectType(connect) => {
                println!("Es un connect");
                let (tx, rx) = sync_channel(MPSC_BUF_SIZE);
                tx.send(Packet::ConnackType(connect.response().clone())).unwrap();
                petition.client.set_pending(rx);
                let _ = server.client_datas
                .insert(petition.client.id.as_ref().unwrap().to_owned(), ClientData::new(tx)).is_none();
                server.queue.enqueue(petition.client);

            }
            _ => todo!()
        }
    }
}

impl Server {
    fn new(port: u16) -> Arc<Self> {
        Arc::new(Self {
            queue: Queue::new(),
            petitions: Petitions::new(),
            client_datas: HashMap::new(),
            port,
        })
    }
    fn next_client(&self) -> Option<Client> {
        self.queue.dequeue()
    }

    fn dispatched(&self, client: Client) {
        self.queue.enqueue(client);
    }

    fn run(mut self: Arc<Self>) {
        let tail = self.queue.tail.clone();
        let _ = thread::spawn(move || {
            listen_for_clients(tail)
        });

        loop {
            let copy = self.clone();

            let handle = thread::spawn(move || dispatch(&copy));
            dispatch(&self);
            handle.join().unwrap();
            println!("Manejando petitions");
            handle_petitions(Arc::get_mut(&mut self).unwrap());
            thread::sleep(SLEEP_DUR)
        }
    }

    fn new_client(&mut self, client: Client, sync_sender: SyncSender<Packet>) {
        self.queue.enqueue(client);
    }

    fn new_petition(&self, client: Client, packet: Packet) {
        self.petitions.sender.send(Petition { client: client, packet }).unwrap();
    }
}

impl Client {
    fn new(stream: TcpStream) -> Client {
        Self {
            id: None,
            stream: stream,
            pending: None,
        }
    }

    fn set_pending(&mut self, pending: Receiver<Packet>) {
        if self.pending.is_some() {
            panic!("Cliente ya tiene pending");
        }
        self.pending = Some(pending);
    }

    fn pending(&self) -> Option<Packet> {
        if let Ok(pending) = self.pending.as_ref().unwrap().try_recv() {
            return Some(pending);
        }
        return None;
    }

    fn new_id(&mut self, id: &str) {
        self.id = Some(id.to_owned())
    }

    //fn send(&mut self, packet: Packet) {
    //    self.stream.write(&packet.as_bytes()).unwrap();
    //}
    /*
    fn receive_packet(&mut self) -> Option<impl Packet> {
        let mut buff: [u8; 1000] = [0; 1000];
        self.stream.read(&mut buff).unwrap();
        return Some(PacketMock::new_mock());
    }
    */
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

fn dispatch(server: &Server) {
    if let Some(mut client) = server.next_client() {
        if let Some(packet) = get_packet(&mut client) {
            println!("habia packet");
            match packet {
                Packet::ConnectType(connect) => {
                    println!("Recibido un Connect, client_id: {}", connect.client_id());
                    client.new_id(connect.client_id());
                    let id = connect.client_id().to_owned();
                    server.new_petition(client, Packet::ConnectType(connect));
                    return;
                }
                _ => todo!()
            }
        }
        println!("llego");
        if let Some(packet) = client.pending() {
            match packet {
                Packet::ConnectType(_connect) => {
                    // No deberia pasar
                    todo!();
                }
                Packet::ConnackType(connack) => {
                    println!("Mandando un CONNACK");
                    client.stream.write_all(&connack.encode()).unwrap();
                }
            }
        }
        println!("despachado");
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
