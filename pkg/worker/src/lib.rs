use connection::{Connection, Status};
use mio::{
    Events, Poll, Token,
    net::{UnixListener, UnixStream},
};

const SERVER: Token = Token(0);
const MAX_SLOTS: usize = 5;

pub fn start(mut socket_dir: String) {
    // get hostname from environment variable or use default
    let hostname = std::env::var("HOST").unwrap_or_else(|_| "worker".to_string());
    socket_dir.extend(format!("/{hostname}.sock").chars());
    let _ = std::fs::remove_file(&socket_dir);
    println!("Starting worker on: {socket_dir}");

    let mut listener = UnixListener::bind(socket_dir).expect("unable to listen on UNIX socket");

    // Performance 10 * 54 max messages per read
    let mut conn_poll: [Connection<540, UnixStream>; MAX_SLOTS] =
        std::array::from_fn(|_| Connection::new(None));

    let mut io_poll = Poll::new().expect("unable to create poll instance");
    let mut events = Events::with_capacity(1024);
    let mut next_token = 1;
    let mut conn_count = 0;

    io_poll
        .registry()
        .register(&mut listener, SERVER, mio::Interest::READABLE)
        .expect("unable to register listener with poll");

    let mut req_count = 0;

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
                                    .register(
                                        &mut stream,
                                        token,
                                        mio::Interest::READABLE | mio::Interest::WRITABLE,
                                    )
                                    .expect("unable to register stream with poll");

                                println!("Accepted connection: {:?}", stream.peer_addr());
                                conn_poll[slot_index].stream = Some(stream);
                                conn_poll[slot_index].status = Status::Readable;
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

                    let _message = match conn.read_messages() {
                        Ok(e) => e,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            println!(
                                "Failed to read message from connection: {e} is better to panic"
                            );
                            continue;
                        }
                    };

                    req_count += 1;
                    print!("Req {req_count}");
                }
            }
        }
    }
}
