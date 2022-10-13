#[macro_use]
extern crate log;

use std::error::Error;
use std::net::TcpStream;
use std::time::Duration;

use clap::Parser;
use rmpv::Value;

use messages::*;
use worker::*;

mod messages;
mod worker;

/// A mocked remote runner worker
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Address in the format IP:PORT
    #[arg(short, long)]
    ip: Option<String>,
    #[arg(short, long)]
    port: Option<String>,
}

fn run_worker(args: Args) -> Result<(), Box<dyn Error>> {
    let stream = TcpStream::connect(format!(
        "{}:{}",
        args.ip.unwrap_or_else(|| "127.0.0.1".to_string()),
        args.port.unwrap_or_else(|| "7042".to_string())
    ))?;

    info!("Connected to {}", stream.peer_addr()?);

    let mut worker = Worker::new(stream);
    worker.set_heartbeat(Some(Duration::from_secs(2)));

    worker.send_message(is_alive_message()?)?;
    worker.request_registration()?;

    worker.start(|worker, message| {
        debug!("Received message: {:?}", &message);
        match message {
            Message::Eval(eval) => match eval.task_context_id() {
                Ok(task_context_id) => {
                    std::thread::sleep(Duration::from_secs(2));
                    worker.send_message(task_result_message(task_context_id)?)?;
                    worker.send_message(next_task_for_worker_message(worker)?)?;
                }
                _ => {}
            },
            Message::Err(_) => {}
            Message::Heartbeat => {
                // on heartbeat send
                worker.send_message(is_alive_message()?)?;
            }
            Message::Registered => {
                worker.send_message(add_observer_message(&worker)?)?;
                worker.send_message(next_task_for_worker_message(&worker)?)?;
            }
            Message::IsAlive(_) => {}
            Message::Enqueue(_) => {}
        }
        Ok(())
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args: Args = Args::parse();

    run_worker(args)
}
