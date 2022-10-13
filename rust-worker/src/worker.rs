use crate::{register_worker_message, Message};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Debug;
use std::io::{BufRead, BufReader, Cursor, ErrorKind, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug)]
pub struct Worker {
    attributes: WorkerAttributes,
    stream: TcpStream,
    heartbeat: Option<Duration>,
    is_registered: bool,
    registration_message_id: Option<String>,
}

impl Worker {
    pub fn new(stream: TcpStream) -> Self {
        let id = Uuid::new_v4().hyphenated().to_string();

        Self {
            attributes: WorkerAttributes::new(id),
            stream,
            heartbeat: None,
            is_registered: false,
            registration_message_id: None,
        }
    }

    pub fn request_registration(&mut self) -> Result<(), Box<dyn Error>> {
        if self.is_registered {
            return Ok(());
        }
        let message = register_worker_message(&self)?;
        self.registration_message_id = Some(message.id().unwrap().to_string());
        self.send_message(message)
    }

    pub fn set_heartbeat(&mut self, heartbeat: Option<Duration>) {
        self.heartbeat = heartbeat;
    }

    pub fn attributes(&self) -> WorkerAttributes {
        self.attributes.clone()
    }

    pub fn id(&self) -> &str {
        self.attributes.id.as_str()
    }

    pub fn is_registered(&self) -> bool {
        self.is_registered
    }

    pub fn send_message(&mut self, message: Message) -> Result<(), Box<dyn Error>> {
        let encoded_message = rmp_serde::to_vec_named(&message)?;
        self.stream.write_all(encoded_message.as_slice())?;
        self.stream.flush()?;

        debug!("Sent message: {:?}", &message);

        Ok(())
    }

    pub fn start(
        mut self,
        handler: fn(&mut Self, message: Message) -> Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let read_stream = self.stream.try_clone()?;
        read_stream.set_read_timeout(self.heartbeat.clone())?;

        let mut reader = BufReader::with_capacity(4096, read_stream);
        loop {
            let amount_of_read_bytes = match reader.fill_buf() {
                Ok([]) => {
                    return Ok(());
                }
                Ok(mut buffer) => {
                    let mut cursor = Cursor::new(buffer);
                    let mut message: Message = match rmp_serde::from_read(&mut cursor) {
                        Ok(val) => val,
                        Err(err) => {
                            eprintln!("Error reading package: {}", err);
                            let unpacked_message = rmpv::decode::read_value_ref(&mut buffer)?;
                            println!("{}", unpacked_message);
                            break;
                        }
                    };

                    if !self.is_registered {
                        if let Some(ref id) = self.registration_message_id {
                            match &message {
                                Message::Eval(eval) => {
                                    if eval.id.as_str() == id.as_str() {
                                        self.is_registered = true;
                                        self.registration_message_id = None;
                                        message = Message::Registered;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    handler(&mut self, message)?;
                    cursor.position() as usize
                }
                Err(error) => match error.kind() {
                    ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                        handler(&mut self, Message::Heartbeat)?;
                        0
                    }
                    _ => {
                        eprintln!("Error filling buffer: {:?}", error);
                        break;
                    }
                },
            };
            reader.consume(amount_of_read_bytes);
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkerAttributes {
    id: String,
    working_directory: PathBuf,
    worker_connection_strategy: WorkerConnectionStrategy,
    platform: String,
    pid: u32,
}

impl WorkerAttributes {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap(),
            worker_connection_strategy: WorkerConnectionStrategy::Single,
            platform: std::env::consts::OS.to_string(),
            id: id.into(),
            pid: std::process::id(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WorkerConnectionStrategy {
    #[serde(rename = "singleConnectionStrategy")]
    Single,
}
