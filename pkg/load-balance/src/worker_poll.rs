use std::{
    io::Write,
    sync::mpsc::{Sender, channel},
};

use message::socket::Message;
use mio::net::UnixStream;

pub struct WorkerPoll {
    socket_dir: String,
    poll: Vec<UnixStream>,
    conn_ptr: usize,
}

impl WorkerPoll {
    pub fn new(socket_dir: String) -> Self {
        WorkerPoll {
            poll: renew(&socket_dir),
            socket_dir: socket_dir.to_owned(),
            conn_ptr: 0,
        }
    }

    pub fn send(&mut self, buf: &[u8]) -> std::io::Result<bool> {
        loop {
            if self.poll.is_empty() {
                self.poll = renew(&self.socket_dir);
                self.conn_ptr = 0;

                if self.poll.is_empty() {
                    panic!("No available worker sockets");
                }
            }

            let mut stream = &self.poll[self.conn_ptr];
            match stream.write(buf) {
                Ok(n) if n == buf.len() => {
                    self.conn_ptr += 1;
                    self.conn_ptr %= self.poll.len();
                    return Ok(true);
                }
                Ok(_) => {
                    self.conn_ptr += 1;
                    return Ok(false);
                }
                Err(e) => {
                    eprintln!("Failed to write to stream: {e}");
                    self.poll.remove(self.conn_ptr);
                }
            }
        }
    }
}

pub fn start_workers(socket_dir: String) -> Sender<Message> {
    let (tx, rx) = channel::<Message>();

    let mut poll = WorkerPoll::new(socket_dir);
    let inner_tx = tx.clone();
    std::thread::spawn(move || {
        let mut retries = 0;
        let mut shutdown = false;
        loop {
            if retries >= 10 && shutdown {
                eprintln!("Worker poll is empty, shutting down");
                break;
            }

            if retries >= 10 {
                println!("Renewing worker sockets after 10 retries");
                poll.poll = renew(&poll.socket_dir);
                retries = 0;
                shutdown = true;
            }

            if let Ok(msg) = rx.recv() {
                if poll.send(&msg.to_bytes()).is_ok() {
                    retries = 0;
                } else {
                    println!("Failed to send message, retrying...");
                    inner_tx.send(msg).unwrap();
                    retries += 1;
                }
            }
        }
    });

    tx
}

fn renew(socket_dir: &str) -> Vec<UnixStream> {
    println!("Renewing worker sockets from folder: {socket_dir}");
    let mut poll = Vec::with_capacity(10);
    for file in std::fs::read_dir(socket_dir).expect("unable to read socket folder") {
        if poll.len() == 10 {
            break;
        }

        let file = if let Ok(file) = file {
            file
        } else {
            continue;
        };

        if file.path().extension() != Some(std::ffi::OsStr::new("sock")) {
            continue;
        }

        if let Ok(mut stream) = UnixStream::connect(file.path())
            && let Ok(n) = stream.write(&Message::Ack.to_bytes())
        {
            println!("Connected to socket: {:?}, sent {} bytes", file.path(), n);
            poll.push(stream);
        } else {
            eprintln!("Failed to connect to socket: {:?}", file.path());
        }
    }

    poll
}
