use std::net::TcpStream;
// use tungstenite::stream::MaybeTlsStream;
// use tungstenite::{connect, Message, WebSocket};
use kryptonite::stream::MaybeTlsStream;
use kryptonite::{connect, Message, WebSocket};
use std::time::{SystemTime,Duration};
use std::sync::mpsc;
use std::thread;

const BINANCE_WS_URL: &str = "wss://stream.binance.com";
//const BINANCE_WS_URL: &str = "wss://stream.testnet.binance.vision:9443";

struct BinanceWebSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    tx: mpsc::Sender<String>,
}

impl BinanceWebSocket {
    fn new(endpoint: &str, tx: mpsc::Sender<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let ws_url = format!("{}/ws/{}",BINANCE_WS_URL, endpoint);
        let (mut socket, _) = connect(&ws_url)?;
        let write_cursor =(&mut socket).context.frame.in_buffer.write_cursor;
        let _ = (&mut socket).context.frame.in_buffer.take_ref(write_cursor);
        Ok(Self { socket, tx })
    }

    fn looping_msg(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            //match self.socket.read(|| {})
            match self.socket.read(|| ()) {
                Ok(Message::Text(txt)) => {
                    if let Err(e) = self.tx.send(txt) {
                        eprintln!("Failed to send message through channel: {}", e);
                        break;
                    }
                }
                Ok(Message::Ping(data)) => {
                    self.socket.send(Message::Pong(data))?;
                }
                Err(e) => {
                    eprintln!("Error reading message: {}", e);
                    break;
                }
                _ => {
                    eprintln!("Unhandled message type");
                }
            }
        }
        Ok(())
    }
}

pub fn run_binance_trades(symbol: &str, tx: mpsc::Sender<String>) {
    let endpoint = format!("{}@trade", symbol.to_lowercase());
    run_binance_stream(&endpoint, tx);
}

pub fn run_binance_orderbook(symbol: &str, depth: usize, tx: mpsc::Sender<String>) {
    let endpoint = format!("{}@depth{}@100ms", symbol.to_lowercase(), depth);
    run_binance_stream(&endpoint, tx);
}

pub fn run_binance_bookticker(symbol: &str, tx: mpsc::Sender<String>) {
    let endpoint = format!("{}@bookTicker", symbol.to_lowercase());
    run_binance_stream(&endpoint, tx);
}

fn run_binance_stream(endpoint: &str, tx: mpsc::Sender<String>) {
    loop {
        match BinanceWebSocket::new(endpoint, tx.clone()) {
            Ok(mut ws) => {
                if let Err(e) = ws.looping_msg() {
                    eprintln!("Error in WebSocket stream for {}: {}", endpoint, e);
                }
            }
            Err(e) => {
                eprintln!("Connection failed for {}: {}", endpoint, e);
            }
        }
        thread::sleep(Duration::from_secs(1)); // Retry delay
    }
}
