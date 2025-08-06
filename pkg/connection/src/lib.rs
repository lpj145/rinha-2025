use std::io::{Read, Write};

use message::socket::Message;

#[derive(PartialEq, Clone)]
pub enum Status {
    Empty,
    Readable,
    Writable,
    Done(bool),
    Close,
}

impl std::fmt::Debug for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Empty => write!(f, "Empty"),
            Status::Readable => write!(f, "Readable"),
            Status::Writable => write!(f, "Writable"),
            Status::Done(success) => write!(f, "Done({success})"),
            Status::Close => write!(f, "Close"),
        }
    }
}

pub struct Connection<const BUFFER_SIZE: usize, T> {
    pub stream: Option<T>,
    pub in_buffer: [u8; BUFFER_SIZE], // buffer for reading
    pub out_buffer: Vec<u8>,          // buffer for writing
    pub status: Status,
    written: usize, // bytes written
    round_trip: usize,
}

// O design dessa coisa teria ficado melhor se ele tivesse levado em conta apenas um modelo de request/response
// o problema é deixar isso performatico
impl<const BUFFER_SIZE: usize, T: Read + Write> Connection<BUFFER_SIZE, T>
where
    T: std::io::Read + std::io::Write,
{
    pub fn new(stream: Option<T>) -> Self {
        Connection {
            stream,
            in_buffer: [0; BUFFER_SIZE],
            out_buffer: Vec::with_capacity(BUFFER_SIZE),
            written: 0,
            status: Status::Empty,
            round_trip: 0,
        }
    }

    pub fn reset(&mut self) {
        self.written = 0;
        self.status = Status::Empty;
        self.round_trip = 0;
        self.out_buffer.clear();
        self.stream = None;
    }

    pub fn read_messages(&mut self) -> std::io::Result<Vec<Message>> {
        if self.stream.is_none() || self.status == Status::Close {
            return Err(std::io::Error::other(
                "Cannot read from a closed connection",
            ));
        }

        let streamref = self.stream.as_mut().unwrap();
        let n = streamref.read(&mut self.in_buffer)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Connection closed",
            ));
        }

        let msg_count = n / Message::SIZE;
        let mut messages: Vec<Message> = Vec::with_capacity(msg_count);
        for i in 0..msg_count {
            match Message::from_bytes(&self.in_buffer[i * Message::SIZE..(i + 1) * Message::SIZE]) {
                Ok(message) => messages.push(message),
                Err(e) => {
                    eprintln!("Failed to parse message: {e}");
                    break;
                }
            }
        }

        Ok(messages)
    }

    pub fn write_messsage(&mut self, message: &Message) -> std::io::Result<()> {
        if self.stream.is_none() || self.status == Status::Close {
            return Err(std::io::Error::other("Cannot write to a closed connection"));
        }

        let streamref = self.stream.as_mut().unwrap();
        self.out_buffer.extend_from_slice(&message.to_bytes());
        self.status = Status::Writable;
        streamref.write_all(&self.out_buffer)
    }

    pub fn http_handle(&mut self, event: &mio::event::Event) -> std::io::Result<&Status> {
        if self.stream.is_none() || self.status == Status::Empty || self.status == Status::Close {
            println!("{:?}", self.status);
            return Err(std::io::Error::other(
                "Connot handle a closed, empty or non stream connections".to_string(),
            ));
        }

        self.round_trip += 1;
        let streamref = self.stream.as_mut().unwrap();
        if self.status == Status::Readable
            && event.is_readable()
            && let Ok(n) = streamref.read(&mut self.in_buffer)
        {
            // println!("{} Reading.. {n}", self.round_trip);
            if n == 0 {
                return Ok(&Status::Close); // Connection closed
            }
            self.status = Status::Writable;
            return Ok(&self.status);
        }

        if self.status == Status::Writable && self.out_buffer.is_empty() {
            // println!(
            //     "{} No response to write, closing connection.",
            //     self.round_trip
            // );
            return Ok(&Status::Done(true)); // Nothing to send back
        }

        if self.status == Status::Writable {
            self.written +=
                streamref.write(&self.out_buffer[self.written..self.out_buffer.len()])?;
            // println!("{} Writing response.. {}", self.round_trip, self.written);
            if self.written == 0 {
                return Ok(&Status::Close); // Connection closed
            }

            if self.written >= self.out_buffer.len() {
                // println!(
                //     "{} Response fully written, closing connection.",
                //     self.round_trip
                // );
                self.status = Status::Done(false);
            }
        }

        if self.status == Status::Done(false) {
            // println!("{} Flushing stream..", self.round_trip);
            streamref.flush()?;
            return Ok(&Status::Done(true));
        }

        Ok(&self.status)
    }
}
