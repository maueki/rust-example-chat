extern crate mio;
use mio::*;

fn main() {

    let poll = Poll::new().unwrap();

    let mut events = Events::with_capacity(1024);

    loop {
        poll.poll(&mut events, None).unwrap();
    }
}
