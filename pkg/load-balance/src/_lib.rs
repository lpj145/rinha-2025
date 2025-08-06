use std::io::{Read, Write};

use mio::{
    Events, Poll, Token,
    event::Event,
    net::{TcpListener, TcpStream},
};

static RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nConnection: Keep-Alive\r\nKeep-Alive: timeout=30, max=500\r\nContent-Length: 98\r\n\r\n{\"default\":{\"totalRequests\": 0,\"totalAmount\": 0},\"fallback\":{\"totalRequests\": 0,\"totalAmount\": 0}}";

struct Connection {
    stream: Option<TcpStream>,
    buffer: [u8; 250], // buffer for reading
    written: usize,    // bytes written
    has_read: bool,    // has read from stream
    has_written: bool, // has written to stream
    round_trip: usize,
}

impl Connection {
    fn new(stream: Option<TcpStream>) -> Self {
        Connection {
            stream,
            buffer: [0; 250],
            written: 0,
            has_read: false,
            has_written: false,
            round_trip: 0,
        }
    }

    fn reset(&mut self) {
        self.written = 0;
        self.has_read = false;
        self.has_written = false;
        self.round_trip = 0;
        self.stream = None;
    }

    fn handle(&mut self, event: &Event) -> std::io::Result<usize> {
        if self.stream.is_none() {
            unreachable!("Connection stream is None, this should not happen");
        }

        let streamref = self.stream.as_mut().unwrap();
        if !self.has_read
            && event.is_readable()
            && let Ok(n) = streamref.read(&mut self.buffer)
        {
            if n == 0 {
                return Ok(0); // Connection closed
            }
            self.has_read = true;
        }

        if self.has_read && !self.has_written {
            self.written += streamref.write(&RESPONSE[self.written..])?;
            if self.written == 0 {
                return Ok(0); // Connection closed
            }

            if self.written >= RESPONSE.len() {
                self.has_written = true;
            }
        }

        if self.has_written {
            streamref.flush()?;
            let _ = streamref.shutdown(std::net::Shutdown::Both);
            return Ok(2);
        }

        Ok(self.has_read as usize + self.has_written as usize)
    }
}

const SERVER: Token = Token(0);
const MAX_SLOTS: usize = 50;

pub fn start(port: u16) {
    let mut listener = TcpListener::bind(format!("0.0.0.0:{port}").parse().unwrap())
        .expect("unable to listen on 9999");

    // Performance
    let mut conn_poll: [Connection; MAX_SLOTS] = std::array::from_fn(|_| Connection::new(None));

    let mut io_poll = Poll::new().expect("unable to create poll instance");
    let mut events = Events::with_capacity(1024);
    let mut next_token = 1;
    let mut conn_count = 0;

    io_poll
        .registry()
        .register(&mut listener, SERVER, mio::Interest::READABLE)
        .expect("unable to register listener with poll");

    loop {
        let mut max_conn_per_iter = 10;
        io_poll.poll(&mut events, None).expect("poll failed");

        for event in &events {
            match event.token() {
                SERVER => {
                    while max_conn_per_iter > 0 && conn_count < MAX_SLOTS {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let token = Token(next_token);
                                let slot_index = next_token % MAX_SLOTS;
                                io_poll
                                    .registry()
                                    .register(&mut stream, token, mio::Interest::READABLE)
                                    .expect("unable to register stream with poll");

                                conn_poll[slot_index].stream = Some(stream);
                                next_token += 1;
                                conn_count += 1;
                                max_conn_per_iter -= 1;
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // No more connections to accept
                                break;
                            }
                            Err(e) => {
                                eprintln!("Error accepting connection: {e}");
                            }
                        }
                    }
                }
                token => {
                    let slot_idx = token.0 % MAX_SLOTS;
                    let conn = conn_poll
                        .get_mut(slot_idx)
                        .expect("no connection found for token");

                    match conn.handle(event) {
                        // Connection closed
                        Ok(2) | Ok(0) => {
                            io_poll
                                .registry()
                                .deregister(conn.stream.as_mut().unwrap())
                                .expect("unable to deregister stream");

                            conn_count -= 1;
                            conn.reset();
                        }
                        Ok(1) => {
                            io_poll
                                .registry()
                                .reregister(
                                    conn.stream.as_mut().unwrap(),
                                    token,
                                    mio::Interest::WRITABLE,
                                )
                                .expect("unable to reregister stream with poll");
                        }
                        Ok(_) => {}
                        // Would block, try again later
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            println!("Would block, try again later: {event:?}");
                            // Since we have tried to read and write, we should reregister for write if we get a block
                            if conn.has_read && !conn.has_written {
                                io_poll
                                    .registry()
                                    .reregister(
                                        conn.stream.as_mut().unwrap(),
                                        token,
                                        mio::Interest::WRITABLE,
                                    )
                                    .expect("unable to reregister stream with poll");
                            }
                        }
                        Err(e) => {
                            eprintln!("Error handling connection: {e}");
                            io_poll
                                .registry()
                                .deregister(conn.stream.as_mut().unwrap())
                                .expect("unable to deregister stream");
                            conn.reset();
                        }
                    }
                }
            }
        }
    }
}
