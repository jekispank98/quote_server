use crate::logger::Logger;
use crate::model::command::Command;
use crate::receiver::QuoteReceiver;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::Duration;
use crate::model::stock_quote::StockQuote;
use crate::model::tickers::Ticker;

pub mod server;

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

    // --- 1. Инициализация Каналов ---

    // Канал для команд от QuoteReceiver: (Command, SocketAddr)
    let (cmd_tx, cmd_rx) = unbounded::<(Command, SocketAddr)>();

    // Канал для сигналов остановки от PingMonitor (через QuoteReceiver)
    let (stop_tx, stop_rx) = unbounded::<SocketAddr>();

    // --- 2. Инициализация Сервера и Монитора ---

    let receiver = QuoteReceiver::new(BIND_ADDRESS)?;

    // Клонируем основной сокет для передачи в потоки стриминга
    let server_socket_clone = receiver.socket.try_clone()?;

    // Запуск приема команд и мониторинга
    // !!! ПРЕДПОЛАГАЕТСЯ, что в receiver.start_with_channel теперь передается stop_tx
    receiver.start_with_channel(stop_tx);

    // --- 3. Хранилище активных стримов ---
    // Ключ: Адрес клиента. Значение: Sender для остановки потока.
    let mut active_streams: HashMap<SocketAddr, Sender<()>> = HashMap::new();

    println!("Сервер запущен и ожидает событий на {}", BIND_ADDRESS);

    // --- 4. ГЛАВНЫЙ ЦИКЛ ОБРАБОТКИ СОБЫТИЙ (Event Loop) ---


    loop {
        select! {
            // СЛУЧАЙ А: Пришла команда от клиента (STREAM, PING и т.д.)
            recv(cmd_rx) -> msg => match msg {
                Ok((cmd, src_addr)) => {
                    match cmd.header.as_str() {
                        "STREAM" => {
                            println!("⚡️ Получена команда STREAM от {}", src_addr);

                            // Проверяем, не стримим ли мы уже этому клиенту
                            if active_streams.contains_key(&src_addr) {
                                println!("Клиент {} уже активен. Игнорируем STREAM.", src_addr);
                                continue;
                            }

                            // 1. Создаем канал для "убийства" нового потока
                            let (shutdown_tx, shutdown_rx) = unbounded::<()>();

                            // 2. Сохраняем "кнопку стоп"
                            active_streams.insert(src_addr, shutdown_tx);

                            // 3. Запускаем поток стриминга
                            let tickers = cmd.tickers.clone();
                            let socket_clone_for_thread = server_socket_clone.try_clone()?;

                            thread::spawn(move || {
                                handle_client_stream(socket_clone_for_thread, src_addr, tickers, shutdown_rx);
                            });
                        },

                        // Команда PING обрабатывается внутри QuoteReceiver, здесь не нужно

                        _ => println!("Неизвестная команда от {}: {}", src_addr, cmd.header),
                    }
                },
                Err(_) => {
                    eprintln!("Канал команд закрыт. Завершение main loop.");
                    break;
                },
            },

            // СЛУЧАЙ Б: Пришел сигнал тайм-аута от PingMonitor
            recv(stop_rx) -> msg => match msg {
                Ok(timeout_addr) => {
                    println!("⚠️ Тайм-аут клиента {}. Останавливаем стриминг...", timeout_addr);

                    // 1. Находим и удаляем "кнопку стоп" из HashMap
                    if let Some(shutdown_tx) = active_streams.remove(&timeout_addr) {
                        // 2. Отправляем сигнал остановки в поток
                        let _ = shutdown_tx.send(());
                        println!("✅ Поток для {} успешно остановлен по Keep-Alive.", timeout_addr);
                    } else {
                        // Это может произойти, если клиент отправил STREAM, но сразу же отвалился,
                        // и PingMonitor обнаружил тайм-аут до того, как поток запустился.
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