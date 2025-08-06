use connection::{Connection, Status};
use mio::{
    Events, Poll, Token,
    net::{TcpListener, TcpStream},
};

use crate::worker_poll::start_workers;

mod worker_poll;

const SERVER: Token = Token(0);
const MAX_SLOTS: usize = 50;

pub fn start(port: u16, socket_dir: String) {
    let mut listener = TcpListener::bind(
        format!("0.0.0.0:{port}")
            .parse()
            .expect("unable to parse socket address"),
    )
    .expect("unable to listen on TCP socket");

    let workers = start_workers(socket_dir);

    // Performance
    let mut conn_poll: [Connection<350, TcpStream>; MAX_SLOTS] =
        std::array::from_fn(|_| Connection::new(None));

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
                                conn_poll[slot_index].status = Status::Readable;
                                next_token += 1;
                                conn_count += 1;
                                max_conn_per_iter -= 1;
                                // println!("Accepted connection");
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

                    match conn.http_handle(event) {
                        Ok(Status::Done(true)) | Ok(Status::Close) => {
                            // println!("Connection closed or done");
                            io_poll
                                .registry()
                                .deregister(conn.stream.as_mut().unwrap())
                                .expect("unable to deregister stream");

                            conn_count -= 1;
                            conn.reset();
                        }
                        Ok(Status::Writable) => {
                            match message::http::Request::from_bytes(&conn.in_buffer) {
                                message::http::Request::Summary(from, to) => {
                                    workers
                                        .send(message::socket::Message::Summary(from, to))
                                        .expect("Failed to send summary message");

                                    conn.out_buffer
                                        .extend_from_slice(message::http::response::SUMMARY);
                                }
                                message::http::Request::Payment(amount, correlation_id) => {
                                    workers
                                        .send(message::socket::Message::Payment(
                                            amount,
                                            correlation_id,
                                        ))
                                        .expect("Failed to send payment message");
                                    // println!(
                                    //     "Payment request for amount {amount} with correlation ID {correlation_id:?}"
                                    // );
                                    conn.out_buffer
                                        .extend_from_slice(message::http::response::OK);
                                }
                                message::http::Request::NotFound => {
                                    conn.out_buffer
                                        .extend_from_slice(message::http::response::NOT_FOUND);
                                }
                                message::http::Request::BadRequest => {
                                    conn.out_buffer
                                        .extend_from_slice(message::http::response::BAD_REQUEST);
                                }
                            }

                            // println!("Connection ready for writing");
                            io_poll
                                .registry()
                                .reregister(
                                    conn.stream.as_mut().unwrap(),
                                    token,
                                    mio::Interest::WRITABLE,
                                )
                                .expect("unable to reregister stream with poll");
                        }
                        Ok(_) => {
                            unreachable!("Unexpected status: {:?}", conn.status);
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            unreachable!("Should not happen, cause mio handles this internally");
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
