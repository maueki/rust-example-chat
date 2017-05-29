#![feature(use_extern_macros)]

extern crate mio;
extern crate http_muncher;
extern crate byteorder;

use std::net::SocketAddr;
use mio::*;
use mio::net::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::fmt;

use http_muncher::{Parser, ParserHandler};

use std::cell::RefCell;
use std::rc::Rc;

mod frame;

use frame::{WebSocketFrame, OpCode};

enum ClientState {
    AwaitingHandshake(RefCell<HttpParser>),
    HandshakeResponse,
    Connected,
}

struct HttpParser {
    current_key: Option<String>,
    headers: Rc<RefCell<HashMap<String, String>>>
}

impl ParserHandler for HttpParser {
    fn on_header_field(&mut self, parser: &mut Parser, s: &[u8]) -> bool {
        self.current_key = Some(std::str::from_utf8(s).unwrap().to_string());
        true
    }

    fn on_header_value(&mut self, parser: &mut Parser, s: &[u8]) -> bool {
        self.headers.borrow_mut()
            .insert(self.current_key.clone().unwrap(),
                    std::str::from_utf8(s).unwrap().to_string());
        true
    }

    fn on_headers_complete(&mut self, parser: &mut Parser) -> bool {
        false
    }
}

struct WebSocketClient {
    socket: TcpStream,
    headers: Rc<RefCell<HashMap<String, String>>>,
    interest: Ready,
    state: ClientState,
}

impl WebSocketClient {
    fn read(&mut self) {
        match self.state {
            ClientState::AwaitingHandshake(_) => {
                self.read_handshake();
            },
            ClientState::Connected => {
                let frame = WebSocketFrame::read(&mut self.socket);
                match frame {
                    Ok(frame) => println!("{:?}", frame),
                    Err(e) => println!("error while reading frame: {}", e)
                }
            },
            _ => {}
        }
    }

    fn read_handshake(&mut self) {

        let mut parser = Parser::request();

        loop {
            let mut buf = [0; 2048];

            match self.socket.read(&mut buf) {
                Err(e) => {
                    println!("Error while reading socket: {:?}", e);
                    return
                },
                Ok(len) => {
                    let is_upgrade = if let ClientState::AwaitingHandshake(ref mut parser_state) = self.state {
                        parser.parse(parser_state.get_mut(), &buf[0..len]);
                        parser.is_upgrade()
                    } else { false };

                    if is_upgrade {
                        println!("is_upgrade()");
                        self.state = ClientState::HandshakeResponse;

                        self.interest.remove(Ready::readable());
                        self.interest.insert(Ready::writable());
                        break;
                    }
                }
            }
        }
    }

    fn write(&mut self) {
        let headers = self.headers.borrow();

        let response_key = gen_key(&headers.get("Sec-WebSocket-Key").unwrap());

        let response = fmt::format(format_args!("HTTP/1.1 101 Switching Protocols\r\n\
                                                 Connection: Upgrade\r\n\
                                                 Sec-WebSocket-Accept: {}\r\n\
                                                 Upgrade: websocket\r\n\r\n", response_key));

        self.socket.write(response.as_bytes()).unwrap();

        self.state = ClientState::Connected;

        self.interest.remove(Ready::writable());
        self.interest.insert(Ready::readable());
    }

    fn new(socket: TcpStream) -> WebSocketClient {
        let headers = Rc::new(RefCell::new(HashMap::new()));

        WebSocketClient {
            socket: socket,
            headers: headers.clone(),
            interest: Ready::readable(),
            state: ClientState::AwaitingHandshake(RefCell::new(HttpParser {
                current_key: None,
                headers: headers.clone()
            })),
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

extern crate sha1;
extern crate rustc_serialize;

use rustc_serialize::base64::{ToBase64, STANDARD};

fn gen_key(key: &String) -> String {
    let mut m = sha1::Sha1::new();
    let mut buf = [0u8; 20];

    m.update(key.as_bytes());
    m.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());

    return m.digest().bytes().to_base64(STANDARD);
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

            if event.readiness().is_readable() {
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
                                      PollOpt::edge() | PollOpt::oneshot()).unwrap();
                    },
                    token => {
                        println!("token: {:?}", token);
                        let mut client = server.clients.get_mut(&token).unwrap();
                        println!("client get_mut");
                        client.read();
                        poll.reregister(&client.socket, token, client.interest,
                                        PollOpt::edge() | PollOpt::oneshot()).unwrap();
                    }
                }
            }

            if event.readiness().is_writable() {
                let mut client = server.clients.get_mut(&event.token()).unwrap();
                client.write();
                poll.reregister(&client.socket, event.token(), client.interest,
                                PollOpt::edge() | PollOpt::oneshot()).unwrap();
            }
        }
    }
}
