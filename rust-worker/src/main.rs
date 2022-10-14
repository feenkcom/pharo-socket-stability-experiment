#[macro_use]
extern crate log;

use std::error::Error;
use std::net::TcpStream;
use std::time::Duration;

use clap::Parser;

use messages::*;
use worker::*;

mod messages;
mod worker;

/// A mocked remote runner worker
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Address in the format IP:PORT
    #[arg(long)]
    ip: Option<String>,
    #[arg(long)]
    port: Option<String>,
    /// Amount of milliseconds between heartbeats
    #[arg(long)]
    heartbeat: Option<u64>,
    /// For how long in milliseconds should the worker simulate task execution
    #[arg(long, default_value_t = 2000)]
    work: u64,
}

fn run_worker(args: Args) -> Result<(), Box<dyn Error>> {
    let stream = TcpStream::connect(format!(
        "{}:{}",
        args.ip.unwrap_or_else(|| "127.0.0.1".to_string()),
        args.port.unwrap_or_else(|| "7042".to_string())
    ))?;

    info!("Connected to {}", stream.peer_addr()?);

    let mut worker = Worker::new(stream);
    worker.set_heartbeat(args.heartbeat.map(|millis| Duration::from_millis(millis)));
    worker.set_work_duration(Some(Duration::from_millis(args.work)));

    worker.send_message(is_alive_message()?)?;
    worker.request_registration()?;

    worker.start(|worker, message| {
        debug!("Received message: {:?}", &message);
        match message {
            Message::Eval(eval) => match eval.task_context_id() {
                Ok(task_context_id) => {
                    worker
                        .work_duration()
                        .map(|duration| std::thread::sleep(duration.clone()));

                    worker.send_message(task_result_message(task_context_id)?)?;
                    worker.send_message(next_task_for_worker_message(worker)?)?;
                }
                _ => {}
            },
            Message::Err(_) => {}
            Message::Heartbeat => {
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
