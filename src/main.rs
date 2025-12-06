use crate::logger::{ConsoleLogger, Logger, MemoryLogger};
use crate::receiver::QuoteReceiver;

pub mod server;

mod error;
mod logger;
pub mod model;
mod receiver;
mod result;

const BIND_ADDRESS: &str = "127.0.0.1:8080";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let receiver = QuoteReceiver::new(BIND_ADDRESS)?;
    let console = Box::new(ConsoleLogger);
    let memory = Box::new(MemoryLogger::new());

    let loggers: Vec<Box<dyn Logger>> = vec![console.clone(), memory];
    let (receiver_handle, quote) = receiver.start_with_channel();

    let mut total_received = 0;
    loop {
        match quote.recv() {
            Ok((quotes, _src_addr)) => {
                total_received += 1;
                println!("quotes: {:?}", quotes)
            }
            Err(_) => {
                console.log("Канал закрыт. Завершение работы.");
                break;
            }
        }
    }
    let _ = receiver_handle.join();
    for logger in &loggers {
        if let Some(mem) = logger.as_any().downcast_ref::<MemoryLogger>() {
            println!("Содержимое MemoryLogger:");
            for entry in mem.get_entries() {
                println!("  {entry}");
            }
        }
    }

    println!("Итог: получено {} пакетов данных", total_received);
    Ok(())
}
