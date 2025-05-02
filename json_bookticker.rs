#[derive(Debug, Copy, Clone)]
pub struct BookTicker {
    pub update_id: u64,
    pub symbol: [u8; 16],  
    pub symbol_len: usize,
    pub bid_price: f64,
    pub bid_quantity: f64,
    pub ask_price: f64,
    pub ask_quantity: f64,
}

impl BookTicker {
    #[inline(always)]
    pub fn new() -> Self {
        BookTicker {
            update_id: 0,
            symbol: [0; 16],
            symbol_len: 0,
            bid_price: 0.0,
            bid_quantity: 0.0,
            ask_price: 0.0,
            ask_quantity: 0.0,
        }
    }
}

#[inline(always)]
unsafe fn parse_number_fast(bytes: &[u8], pos: &mut usize) -> f64 {
    let mut int_part = 0u64;
    let mut frac_part = 0u64;
    let mut frac_digits = 0u32;
    
    // Skip to the first digit after quote
    *pos += 2; // Skip colon and quote
    
    // Integer part
    while *bytes.get_unchecked(*pos) != b'.' {
        int_part = (int_part << 3) + (int_part << 1) + (*bytes.get_unchecked(*pos) & 0x0f) as u64;
        *pos += 1;
    }
    *pos += 1; // skip decimal
    
    // Fractional part
    while *bytes.get_unchecked(*pos) != b'"' {
        frac_part = (frac_part << 3) + (frac_part << 1) + (*bytes.get_unchecked(*pos) & 0x0f) as u64;
        frac_digits += 1;
        *pos += 1;
    }
    
    *pos += 1; // skip closing quote
    
    const POW10: [f64; 9] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8];
    int_part as f64 + (frac_part as f64 / POW10[frac_digits as usize])
}

#[inline(always)]
unsafe fn parse_u64_fast(bytes: &[u8], pos: &mut usize) -> u64 {
    let mut result = 0u64;
    
    *pos += 1; // Skip colon
    
    while *bytes.get_unchecked(*pos) >= b'0' && *bytes.get_unchecked(*pos) <= b'9' {
        result = (result << 3) + (result << 1) + (*bytes.get_unchecked(*pos) & 0x0f) as u64;
        *pos += 1;
    }
    
    result
}

#[inline(always)]
unsafe fn parse_symbol_fast(bytes: &[u8], pos: &mut usize, symbol: &mut [u8], len: &mut usize) {
    *pos += 2; // Skip colon and opening quote
    
    *len = 0;
    while *bytes.get_unchecked(*pos) != b'"' {
        *symbol.get_unchecked_mut(*len) = *bytes.get_unchecked(*pos);
        *len += 1;
        *pos += 1;
    }
    *pos += 1; // skip closing quote
}

#[inline(always)]
pub fn parse_book_ticker(data: &[u8], ticker: &mut BookTicker) {
    let mut pos = 4; // Skip {"u":
    
    unsafe {
        // Parse update_id
        ticker.update_id = parse_u64_fast(data, &mut pos);
        pos += 4; // Skip comma,"s"
        
        // Parse symbol
        parse_symbol_fast(data, &mut pos, &mut ticker.symbol, &mut ticker.symbol_len);
        pos += 4; // Skip comma,"b"
        
        // Parse bid price
        ticker.bid_price = parse_number_fast(data, &mut pos);
        pos += 4; // Skip comma,"B"
        
        // Parse bid quantity
        ticker.bid_quantity = parse_number_fast(data, &mut pos);
        pos += 4; // Skip comma,"a"
        
        // Parse ask price
        ticker.ask_price = parse_number_fast(data, &mut pos);
        pos += 4; // Skip comma,"A"
        
        // Parse ask quantity
        ticker.ask_quantity = parse_number_fast(data, &mut pos);
    }
}
