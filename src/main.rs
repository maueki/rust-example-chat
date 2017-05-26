extern crate mio;
extern crate http_muncher;

use std::net::SocketAddr;
use mio::*;
use mio::net::*;
use std::collections::HashMap;
use std::io::Read;

use http_muncher::{Parser, ParserHandler};

struct HttpParser;
impl ParserHandler for HttpParser {}

struct WebSocketClient {
    socket: TcpStream,
    http_parser: Parser
}

impl WebSocketClient {
    fn read(&mut self) {
        loop {
            let mut buf = [0; 2048];
            match self.socket.read(&mut buf) {
                Err(e) => {
                    println!("Error while reading socket: {:?}", e);
                    return
                },
                Ok(len) => {
                    let mut parser_handler = HttpParser{};
                    self.http_parser.parse(&mut parser_handler, &buf[0..len]);
                    if self.http_parser.is_upgrade() {
                        println!("is_upgrade()");
                        break;
                    }
                }
            }
        }
    }

    fn new(socket: TcpStream) -> WebSocketClient {
        WebSocketClient {
            socket: socket,
            http_parser: Parser::request()
        }
    }
}

struct WebSocketServer {
    socket: TcpListener,
    clients: HashMap<Token, WebSocketClient>,
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
            println!("event: {:?}", event);
            match event.token() {
                SERVER => {
                    println!("SERVER ready!");
                    let client = match server.socket.accept() {
                        Err(e) => panic!("Error during accept(): {}", e),
                        Ok((sock, _addr)) => sock
                    };
                    server.token_counter += 1;
                    let new_token = Token(server.token_counter);
                    server.clients.insert(new_token, WebSocketClient::new(client));
                    poll.register(&server.clients[&new_token].socket, new_token, Ready::readable(),
                                  PollOpt::edge() /* | PollOpt::oneshot() */).unwrap();
                },
                token => {
                    println!("token: {:?}", token);
                    let mut client = server.clients.get_mut(&token).unwrap();
                    println!("client get_mut");
                    client.read();
//                    poll.register(&client.socket, token, Ready::readable(),
//                                  PollOpt::edge() | PollOpt::oneshot()).unwrap();
                }
            }
        }
    }
}
