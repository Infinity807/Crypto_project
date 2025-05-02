use jsonic;

// pub fn process_bid_ask_jsonic(json_data: &str) -> (String, String) {
//     // Parse the JSON using JSONIC
//     let parsed = jsonic::parse(json_data).unwrap();

//     // Extract the first bid and ask
//     let bids = &parsed["bids"];
//     let asks = &parsed["asks"];
//     let first_bid = &bids[0][0].as_str().unwrap();
//     let first_ask = &asks[0][0].as_str().unwrap();

//     // Convert to owned strings
//     (first_bid.to_string(), first_ask.to_string())

// }

pub fn process_bid_ask_jsonic(json_data: &str) -> (f64, f64) {
    let parsed = jsonic::parse(json_data).unwrap();

    // Extract the first bid and ask as strings then parse to int 
    let bids = &parsed["bids"];
    let asks = &parsed["asks"];
    let first_bid = bids[0][0].as_str().unwrap().parse::<f64>().unwrap();
    let first_ask = asks[0][0].as_str().unwrap().parse::<f64>().unwrap();

    (first_bid, first_ask)
}

pub fn process_bookTicker(json_data: &str) -> (f64, f64) {
    let parsed = jsonic::parse(json_data).unwrap();

    // Extract the first bid and ask as strings then parse to int 
    let bids = &parsed["b"];
    let asks = &parsed["a"];
    let first_bid = bids.as_str().unwrap().parse::<f64>().unwrap();
    let first_ask = asks.as_str().unwrap().parse::<f64>().unwrap();

    (first_bid, first_ask)
}




