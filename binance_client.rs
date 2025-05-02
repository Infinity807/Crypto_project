use chrono::Utc;
use log::{info, error};
use serde_json::{json, Value};
use std::fs::File;
use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
//use tungstenite::{connect, Message};
use ed25519_dalek::{Signer, SigningKey};
use base64::{Engine as _, engine::general_purpose::STANDARD as base64};
use kryptonite::{connect, Message};

pub struct BinanceWSClient {
    socket: kryptonite::WebSocket<kryptonite::stream::MaybeTlsStream<std::net::TcpStream>>,
    //socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    api_key: String,
    private_key: SigningKey,
}

#[derive(Debug)]
pub enum BinanceError {
    IoError(std::io::Error),
    // WebSocketError(tungstenite::Error),
    WebSocketError(kryptonite::Error),
    JsonError(serde_json::Error),
    KeyError(String),
    AuthError(String),
}

impl From<std::io::Error> for BinanceError {
    fn from(err: std::io::Error) -> Self {
        BinanceError::IoError(err)
    }
}

// impl From<tungstenite::Error> for BinanceError {
//     fn from(err: tungstenite::Error) -> Self {
//         BinanceError::WebSocketError(err)
//     }
impl From<kryptonite::Error> for BinanceError {
    fn from(err: kryptonite::Error) -> Self {
        BinanceError::WebSocketError(err)
    }
}

impl From<serde_json::Error> for BinanceError {
    fn from(err: serde_json::Error) -> Self {
        BinanceError::JsonError(err)
    }
}

impl BinanceWSClient {
    pub fn new(api_key: &str, private_key_path: &str) -> Result<Self, BinanceError> {
        // Load private key
        let private_key = Self::load_private_key(private_key_path)?;
        
        //wss://ws-api.binance.com:443/ws-api/v3
        // Connect to WebSocket
        //let url = Url::parse("wss://testnet.binance.vision:443/ws-api/v3").unwrap();
        let ws_url: String = format!("wss://testnet.binance.vision:443/ws-api/v3");
        //let (socket, response) = connect(url)?;
        let (mut socket, response) = connect(&ws_url)?;
        // mut socket 
        let write_cursor =(&mut socket).context.frame.in_buffer.write_cursor;
        let _ = (&mut socket).context.frame.in_buffer.take_ref(write_cursor);
        info!("WebSocket connection established: {:?}", response);

        let mut client = Self {
            socket,
            api_key: api_key.to_string(),
            private_key,
        };

        // Perform authentication
        client.authenticate()?;

        Ok(client)
    }

    fn load_private_key(private_key_path: &str) -> Result<SigningKey, BinanceError> {
        let mut key_file = File::open(private_key_path)?;
        let mut key_content = String::new();
        key_file.read_to_string(&mut key_content)?;
        
        let pem_lines: Vec<&str> = key_content.lines().collect();
        let key_base64: String = pem_lines
            .iter()
            .filter(|line| !line.contains("BEGIN") && !line.contains("END"))
            .map(|line| line.trim())
            .collect();
        
        let key_bytes = base64.decode(key_base64)
            .map_err(|e| BinanceError::KeyError(e.to_string()))?;
        
        let key_bytes: [u8; 32] = key_bytes[key_bytes.len()-32..]
            .try_into()
            .map_err(|_| BinanceError::KeyError("Failed to extract 32 bytes for key".to_string()))?;
        
        let signing_key = SigningKey::from_bytes(&key_bytes);
        info!("Private key loaded successfully");
        Ok(signing_key)
    }

    fn generate_signature(&self, payload: &str) -> String {
        let signature = self.private_key.sign(payload.as_bytes());
        base64.encode(signature.to_bytes())
    }

    fn authenticate(&mut self) -> Result<(), BinanceError> {
        let timestamp = Utc::now().timestamp_millis();
        let params = format!("apiKey={}&timestamp={}", self.api_key, timestamp);
        let signature = self.generate_signature(&params);

        let auth_request = json!({
            "id": format!("auth_{}", timestamp),
            "method": "session.logon",
            "params": {
                "apiKey": self.api_key,
                "timestamp": timestamp,
                "signature": signature
            }
        });

        self.socket.send(Message::Text(auth_request.to_string()))?;
        info!("Authentication request sent");
        //||{} ko urana hai
        if let Message::Text(response_text) = self.socket.read(||{})? {
            let response: Value = serde_json::from_str(&response_text)?;
            
            if response["status"] == 200 {
                info!("Authentication successful");
                Ok(())
            } else {
                error!("Authentication failed: {:?}", response);
                Err(BinanceError::AuthError("Authentication failed".to_string()))
            }
        } else {
            Err(BinanceError::AuthError("Invalid response format".to_string()))
        }
    }

    // order.test with computeCommissionRates
    //implemenmt difference types
    pub fn place_order(&mut self, symbol: &str, side: &str, order_type: &str, 
                      price: &str, quantity: &str) -> Result<Value, BinanceError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        let params = json!({
            "symbol": symbol,
            "side": side,
            "type": order_type,
            "price": price,
            "quantity": quantity,
            "timeInForce": "GTC",
            "timestamp": timestamp
        });

        let request = json!({
            "id": format!("order_{}", timestamp),
            "method": "order.place",
            "params": params
        });

        self.socket.send(Message::Text(request.to_string()))?;
        info!("Order request sent: {:?}", request);
        //||() ko urana hai
        if let Message::Text(response_text) = self.socket.read(||())? {
            let response: Value = serde_json::from_str(&response_text)?;
            info!("Order response received: {:?}", response);
            Ok(response)
        } else {
            Err(BinanceError::AuthError("Invalid response format".to_string()))
        }
    }

    pub fn cancel_order(&mut self, symbol: &str, order_id: &str) -> Result<Value, BinanceError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let params = json!({
            "symbol": symbol,
            "orderId": order_id,
            "timestamp": timestamp
        });

        let request = json!({
            "id": format!("cancel_{}", timestamp),
            "method": "order.cancel",
            "params": params
        });

        self.socket.send(Message::Text(request.to_string()))?;
        info!("Cancel order request sent: {:?}", request);
        //||() urana hai
        if let Message::Text(response_text) = self.socket.read(||())? {
            let response: Value = serde_json::from_str(&response_text)?;
            info!("Cancel order response received: {:?}", response);
            Ok(response)
        } else {
            Err(BinanceError::AuthError("Invalid response format".to_string()))
        }
    }
//<TODO>
    pub fn acc_status(&mut self) -> Result<Value, BinanceError>{
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let params = json!({
            "timestamp": timestamp
        });

        let request = json!({
            "id": format!("status_{}", timestamp),
            "method": "account.status",
            "params": params
        });

        self.socket.send(Message::Text(request.to_string()))?;
        info!("Acc Status request sent: {:?}", request);
        //||() urana hai
        if let Message::Text(response_text) = self.socket.read(||())? {
            let response: Value = serde_json::from_str(&response_text)?;
            info!("Account status received: {:?}", response);
            Ok(response)
        } else {
            Err(BinanceError::AuthError("Invalid response format".to_string()))
        }
    }
        // {
        //     "id": "734235c2-13d2-4574-be68-723e818c08f3",
        //     "method": "allOrders",
        //     "params": {
        //     "symbol": "BTCUSDT",
        //     "startTime": 1660780800000,
        //     "endTime": 1660867200000,
        //     "limit": 5,
        //     "apiKey": "vmPUZE6mv9SD5VNHk4HlWFsOr6aKE2zvsw0MuIgwCIPy6utIco14y7Ju91duEh8A",
        //     "signature": "f50a972ba7fad92842187643f6b930802d4e20bce1ba1e788e856e811577bd42",
        //     "timestamp": 1661955123341
        //     }
        // }

    // pub fn user_data(&mut self,symbol: &str,start_time: &Duration, end_time: &Duration) -> Result<Value, BinanceError>{
    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis();

    //     let params = json!({
    //         "symbol": "BTCUSDT",
    //         "startTime": start_time,
    //         "endTime": end_time,
    //         "limit": 5,
    //         "timestamp": timestamp
    //     });

    // }

//</TODO>

    pub fn close(mut self) -> Result<(), BinanceError> {
        self.socket.close(None)?;
        info!("WebSocket connection closed");
        Ok(())
    }


}