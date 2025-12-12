use crate::logger::Logger;
use crate::model::stock_quote::StockQuote;
use crate::model::tickers::Ticker;
use crate::receiver::QuoteReceiver;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::Duration;

pub mod sender;

mod error;
mod logger;
pub mod model;
mod receiver;
mod result;


// --- КОНСТАНТЫ ---
const BIND_ADDRESS: &str = "0.0.0.0:8080";
const PING_TIMEOUT: Duration = Duration::from_secs(5);


// =================================================================
// 1. ФУНКЦИЯ ОБРАБОТКИ СТРИМИНГА (handle_client_stream)
// Это та функция, которую мы обсуждали, она запускается в отдельном потоке.
// =================================================================

/// Обрабатывает отправку котировок одному клиенту в отдельном потоке.
pub fn handle_client_stream(
    socket: UdpSocket,
    target_addr: SocketAddr,
    tickers: Vec<Ticker>,
    stop_rx: Receiver<()>
) {
    println!("▶️ Запущен поток стриминга для клиента: {}", target_addr);

    let send_interval = Duration::from_millis(1000); // 1 секунда

    loop {
        // Проверка сигнала остановки (Keep-Alive)
        if let Ok(_) = stop_rx.try_recv() {
            println!("🛑 Остановка стриминга для {} по сигналу тайм-аута.", target_addr);
            break;
        }

        // Генерация и отправка данных
        for ticker in &tickers {
            match StockQuote::generate_new(ticker) {
                Ok(quote) => {
                    let data = quote.to_bytes();
                    if let Err(e) = socket.send_to(&data, target_addr) {
                        eprintln!("❌ Ошибка отправки данных клиенту {}: {}. Прерывание потока.", target_addr, e);
                        return; // Завершаем поток при ошибке отправки
                    }
                },
                Err(e) => {
                    eprintln!("⚠️ Ошибка генерации котировки для {:?}: {:?}", ticker, e);
                }
            }
        }

        // Пауза между отправками
        thread::sleep(send_interval);
    }

    println!("✅ Поток стриминга для {} завершен.", target_addr);
}


// =================================================================
// 2. ОСНОВНАЯ ФУНКЦИЯ (main)
// Главный цикл Event Loop с обработкой команд и тайм-аутов.
// =================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Запуск UDP Сервера Котировок ---");
    let (stop_tx, stop_rx) = unbounded::<SocketAddr>();
    let receiver = QuoteReceiver::new(BIND_ADDRESS)?;
    let server_socket_clone = receiver.socket.try_clone()?;
    let (receiver_thread_handle, cmd_rx)  = receiver.start_with_channel(stop_tx);
    let mut active_streams: HashMap<SocketAddr, Sender<()>> = HashMap::new();

    println!("Сервер запущен и ожидает событий на {}", BIND_ADDRESS);

    loop {
        select! {
            recv(cmd_rx) -> msg => match msg {
                Ok((cmd, src_addr)) => {
                    println!("command: {:?}", cmd);
                    match cmd.header.as_str() {
                        "J_QUOTE" => {
                            println!("⚡️ Получен запрос котировок от {}", src_addr);
                            if active_streams.contains_key(&src_addr) {
                                println!("Клиент {} уже активен. Игнорируем STREAM.", src_addr);
                                continue;
                            }
                            let (shutdown_tx, shutdown_rx) = unbounded::<()>();
                            active_streams.insert(src_addr, shutdown_tx);

                            let tickers = cmd.tickers.clone();
                            let socket_clone_for_thread = server_socket_clone.try_clone()?;

                            thread::spawn(move || {
                                handle_client_stream(socket_clone_for_thread, src_addr, tickers, shutdown_rx);
                            });
                        },

                        _ => println!("Неизвестная команда от {}: {}", src_addr, cmd.header),
                    }
                },
                Err(_) => {
                    eprintln!("Канал команд закрыт. Завершение main loop.");
                    break;
                },
            },
            recv(stop_rx) -> msg => match msg {
                Ok(timeout_addr) => {
                    println!("⚠️ Тайм-аут клиента {}. Останавливаем стриминг...", timeout_addr);
                    if let Some(shutdown_tx) = active_streams.remove(&timeout_addr) {
                        let _ = shutdown_tx.send(());
                        println!("✅ Поток для {} успешно остановлен по Keep-Alive.", timeout_addr);
                    } else {
                        println!("Клиент {} не найден в активных стримах, но получил тайм-аут.", timeout_addr);
                    }
                },
                Err(_) => {
                    eprintln!("Канал остановки закрыт. Завершение main loop.");
                    break;
                },
            },
        }
    }

    Ok(())
}