use url::Url;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
use reqwest::blocking::Client;
use serde_json::Value;

//const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443";
const BINANCE_WS_URL: &str = "wss://testnet.binance.vision";

const PING_INTERVAL: Duration = Duration::from_secs(30);

struct BinanceUserDataStream {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    tx: mpsc::Sender<String>,
    api_key: String,
    listen_key: String,
    last_ping: SystemTime,
}

impl BinanceUserDataStream {
    fn new(api_key: String, tx: mpsc::Sender<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let listen_key = Self::get_listen_key(&api_key)?;
        let url = Url::parse(&format!("{}/ws/{}", BINANCE_WS_URL, listen_key))?;
        let (socket, _) = connect(url)?;

        Ok(Self {
            socket,
            tx,
            api_key,
            listen_key,
            last_ping: SystemTime::now(),
        })
    }

    fn get_listen_key(api_key: &str) -> Result<String, Box<dyn std::error::Error>> {
        let client = Client::new();
        let response: Value = client
            //.post("https://api.binance.com/api/v3/userDataStream")
            .post("https://testnet.binance.vision/api/v3/userDataStream")
            .header("X-MBX-APIKEY", api_key)
            .send()?
            .json()?;

        if let Some(listen_key) = response["listenKey"].as_str() {
            println!("Listen key:{}",listen_key);
            return Ok(listen_key.to_string());
        }

        Err("Failed to extract listen key".into())
    }

    fn ping_listen_key(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        client
            .put(&format!(
                "https://api.binance.com/api/v3/userDataStream?listenKey={}",
                self.listen_key
            ))
            .header("X-MBX-APIKEY", &self.api_key)
            .send()?;

        Ok(())
    }

//     fn handle_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
//         loop {
//             if SystemTime::now().duration_since(self.last_ping)? >= PING_INTERVAL {
//                 if let Err(e) = self.ping_listen_key() {
//                     eprintln!("Failed to ping listen key: {}", e);
//                     return Ok(()); // Trigger reconnect
//                 }
//                 self.last_ping = SystemTime::now();
//             }

//             self.socket.get_ref().get_ref().set_read_timeout(Some(Duration::from_secs(1)))?;
            
//             match self.socket.read() {
//                 Ok(Message::Text(msg)) => {
//                     if self.tx.send(msg).is_err() {
//                         eprintln!("Channel send error");
//                         return Err("Channel send error".into());
//                     }
//                 }
//                 Ok(Message::Ping(data)) => {
//                     self.socket.send(Message::Pong(data))?;
//                 }
//                 Ok(Message::Close(_)) => {
//                     return Ok(());
//                 }
//                 Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
//                     continue;
//                 }
//                 Err(e) => {
//                     eprintln!("WebSocket error: {}", e);
//                     return Ok(());
//                 }
//                 _ => {}
//             }
//         }
//     }
// }


fn handle_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if SystemTime::now().duration_since(self.last_ping)? >= PING_INTERVAL {
            if let Err(e) = self.ping_listen_key() {
                eprintln!("Failed to ping listen key: {}", e);
                return Ok(()); // Trigger reconnect
            }
            self.last_ping = SystemTime::now();
        }

        // Set read timeout on the underlying stream
        match self.socket.get_ref() {
            MaybeTlsStream::Plain(stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(1)))?;
            }
            MaybeTlsStream::NativeTls(stream) => {
                stream.get_ref().set_read_timeout(Some(Duration::from_secs(1)))?;
            }
            _ => {}
        }

        match self.socket.read() {
            Ok(Message::Text(msg)) => {
                if self.tx.send(msg).is_err() {
                    eprintln!("Channel send error");
                    return Err("Channel send error".into());
                }
            }
            Ok(Message::Ping(data)) => {
                self.socket.send(Message::Pong(data))?;
            }
            Ok(Message::Close(_)) => {
                return Ok(());
            }
            Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                return Ok(());
            }
            _ => {}
        }
    }
}
}

pub fn run_binance_user_data_stream(api_key: String, tx: mpsc::Sender<String>) {
    loop {
        match BinanceUserDataStream::new(api_key.clone(), tx.clone()) {
            Ok(mut stream) => {
                if let Err(e) = stream.handle_stream() {
                    eprintln!("Stream error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to create stream: {}", e);
            }
        }

        thread::sleep(Duration::from_secs(5));
    }
}
