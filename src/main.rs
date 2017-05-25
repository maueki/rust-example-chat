extern crate mio;

use std::net::SocketAddr;
use mio::*;
use mio::net::*;
use std::collections::HashMap;
use std::io::Read;

struct WebSocketServer {
    socket: TcpListener,
    clients: HashMap<Token, TcpStream>,
    token_counter: usize
}

impl WebSocketServer {
    fn new(socket: TcpListener) -> WebSocketServer {
        WebSocketServer{
            socket: socket,
            clients: HashMap::new(),
            token_counter: 0
        }
    }
}

fn main() {

    const SERVER: Token = Token(0);

    let addr = "0.0.0.0:10000".parse::<SocketAddr>().unwrap();
    let socket = TcpListener::bind(&addr).unwrap();

    let mut server = WebSocketServer::new(socket);

    let poll = Poll::new().unwrap();

    poll.register(&server.socket, SERVER, Ready::readable(),
                  PollOpt::edge()).unwrap();

    let mut events = Events::with_capacity(1024);

    loop {
        poll.poll(&mut events, None).unwrap();

        for event in events.iter() {
            match event.token() {
                SERVER => {
                    println!("SERVER ready!");
                    let client = match server.socket.accept() {
                        Err(e) => panic!("Error during accept(): {}", e),
                        Ok((sock, _addr)) => sock
                    };
                    server.token_counter += 1;
                    let new_token = Token(server.token_counter);
                    server.clients.insert(new_token, client);
                    poll.register(&server.clients[&new_token], new_token, Ready::readable(),
                                  PollOpt::edge() | PollOpt::oneshot()).unwrap();
                },
                token => {
                    let mut client = server.clients.get_mut(&token).unwrap();
                    let mut buf = [0; 2048];
                    match client.read(&mut buf) {
                        Err(e) => {
                            println!("Error while reading socket: {:?}", e);
                            return
                        },
                        Ok(len) => {
                            println!("client read: {}", String::from_utf8_lossy(&buf[0..len]));
                        }
                    }
                }
            }
        }
    }
}
