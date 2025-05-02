use std::sync::mpsc;
use std::thread;
use log::info;
use std::collections::VecDeque;
mod binance_data;
mod json_parse;
mod user_stream;
use user_stream::run_binance_user_data_stream;
mod binance_client;
use std::time::{SystemTime, UNIX_EPOCH};
use binance_client::{BinanceWSClient, BinanceError};

fn process_market_data_and_orders(
    client: &mut BinanceWSClient, 
    bid: f64, 
    ask: f64
) -> Result<(), BinanceError> {
    // Your order logic here
    // For example:
    let mid_price = (bid + ask) / 2.0;
    let new_bid_price = ask * 1.0; // Place bid 0.1% below mid
    let rounded_price = format!("{:.4}", ask+0.01); // Round to 4 decimal places
    //let rounded_price: f64 = (new_bid_price * 10_000.0).round() / 10_000.0; // Round to 4 decimal places

    let quantity = "10";
    
    // Place new order
    info!("Placing new order at {}", new_bid_price);
    let order_response = client.place_order(
        "USUALUSDT",
        "BUY",
        "LIMIT",
        &rounded_price.to_string(),
        quantity
    )?;
    
    //info!("New order placed: {:?}", order_response);
    Ok(())
}

fn main()->Result<(), BinanceError> {
    simple_logger::init_with_level(log::Level::Info).unwrap();

    //Create new client instance
    let mut client = BinanceWSClient::new(
        "vmob6iiBXHGmdGLhPk5KagEwWUltbSRUNBHXlzU6bW0ge2Z88YuaVZ43JQaH7Zlq",
        "src/private_key.pem"
    )?;
    let (trades_tx, trades_rx) = mpsc::channel();
    let (usdtfdusd_orderbook_tx, usdtfdusd_orderbook_rx) = mpsc::channel();
    let (btcusdt_bookticker_tx, btcusdt_bookticker_rx) = mpsc::channel();
    let (btcfdusd_orderbook_tx, btcfdusd_orderbook_rx) = mpsc::channel();
    let (tx, rx) = mpsc::channel();
    //user_stream_thread
    thread::spawn(move || {
        run_binance_user_data_stream("vmob6iiBXHGmdGLhPk5KagEwWUltbSRUNBHXlzU6bW0ge2Z88YuaVZ43JQaH7Zlq".to_string(), tx);
    });

    // BTCUSDT trades thread
    let btcusdt_trades_tx = trades_tx.clone();
    thread::spawn(move || { 
        binance_data::run_binance_trades("btcusdt", btcusdt_trades_tx);  // run ko change karke binance run trades,order book dept etc.
    });

    // BTCFDUSD trades thread
    let btcfdusd_trades_tx = trades_tx.clone();
    thread::spawn(move || {
        binance_data::run_binance_trades("btcfdusd",btcfdusd_trades_tx);
    });

    // USDTFDUSD trades thread
    let usdtfdusd_trades_tx = trades_tx.clone();
    thread::spawn(move || {
        binance_data::run_binance_trades("fdusdusdt", usdtfdusd_trades_tx);
    });

    //BTCUSDT orderbook thread
    let btcusdt_bookticker_tx = btcusdt_bookticker_tx.clone();
    thread::spawn(move || {
        binance_data::run_binance_bookticker("usualusdt", btcusdt_bookticker_tx);  // run ko change karke binance run orderbook,order book dept etc.
    });

    // BTCFDUSD orderbook thread
    let btcfdusd_orderbook_tx = btcfdusd_orderbook_tx.clone();
    thread::spawn(move || {
        binance_data::run_binance_orderbook("usualusdt", 5, btcfdusd_orderbook_tx);
    });

    // USDTFDUSD orderbook thread
    let usdtfdusd_orderbook_tx = usdtfdusd_orderbook_tx.clone();
    thread::spawn(move || {
        binance_data::run_binance_orderbook("usualfdusd",5,  usdtfdusd_orderbook_tx);
    });
    

    loop {
        if let Ok(msg) = rx.try_recv(){
            println!("Exectution report: {}",msg);
        }
         if let Ok(msg) = trades_rx.try_recv() {
            println!("Trade message: {}", msg);
        }
        if let Ok(msg) = btcusdt_bookticker_rx.try_recv() {
            let (bid, ask) = json_parse::process_bookTicker(&msg);
            println!("bookticker ka USDTFDUSD Bid hai: {}   Ask hai: {}", bid, ask);
            let paisa = format!("{:.4}", ((ask-bid)/((ask+bid)/2.0))*100.0);
            println!("Difference hai {}", ask-bid);
            println!("Message hai : {} ", paisa);
        }
        

        if let Ok(msg) = btcfdusd_orderbook_rx.try_recv() {
            let (bid, ask) = json_parse::process_bid_ask_jsonic(&msg);
            println!("BTCFDUSD Bid: {}    BTCFDUSD Ask: {} msg:{}", bid, ask, msg);
            process_market_data_and_orders(&mut client, bid, ask);
            }       

        //client.acc_status();
         if let Ok(msg) = usdtfdusd_orderbook_rx.try_recv() {
             let (bid, ask) = json_parse::process_bid_ask_jsonic(&msg);
            println!("USDTFDUSD Bid hai: {}   Ask hai: {}", bid, ask);
            println!("Message hai : {} ", msg);
        }
    }
}
