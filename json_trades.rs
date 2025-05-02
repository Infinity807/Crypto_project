use std::hint::black_box;
use std::time::Instant;

#[derive(Debug, Copy, Clone)]
pub struct Trade {
    pub symbol: [u8; 16],
    pub symbol_len: usize,
    pub price: f64,
    pub quantity: f64,
    pub is_maker: bool,
    pub timestamp: u64,  // Added timestamp field
}

impl Trade {
    #[inline(always)]
    pub fn new() -> Self {
        Trade {
            symbol: [0; 16],
            symbol_len: 0,
            price: 0.0,
            quantity: 0.0,
            is_maker: false,
            timestamp: 0,  // Initialize timestamp
        }
    }
}

#[inline(always)]
unsafe fn parse_number_fast(bytes: &[u8], pos: &mut usize) -> f64 {
    let mut int_part = 0u64;
    let mut frac_part = 0u64;
    let mut frac_digits = 0u32;
    
    // Skip to value
    while *bytes.get_unchecked(*pos) != b':' {
        *pos += 1;
    }
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
unsafe fn parse_symbol_fast(bytes: &[u8], pos: &mut usize, symbol: &mut [u8], len: &mut usize) {
    while *bytes.get_unchecked(*pos) != b':' {
        *pos += 1;
    }
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
unsafe fn parse_bool_fast(bytes: &[u8], pos: &mut usize) -> bool {
    while *bytes.get_unchecked(*pos) != b':' {
        *pos += 1;
    }
    *pos += 1; // Skip colon
    
    let result = *bytes.get_unchecked(*pos) == b't';
    *pos += if result { 4 } else { 5 }; // Skip "true" or "false"
    result
}

#[inline(always)]
unsafe fn parse_timestamp_fast(bytes: &[u8], pos: &mut usize) -> u64 {
    let mut value = 0u64;
    
    // Skip to value
    while *bytes.get_unchecked(*pos) != b':' {
        *pos += 1;
    }
    *pos += 1; // Skip colon
    
    // Parse integer
    while *bytes.get_unchecked(*pos) != b',' {
        value = (value << 3) + (value << 1) + (*bytes.get_unchecked(*pos) & 0x0f) as u64;
        *pos += 1;
    }
    
    value
}

#[inline(always)]
pub fn parse_trade(data: &[u8], trade: &mut Trade) {
    let mut pos = 2; // Skip directly to after {"
    
    unsafe {
        // Skip "e" field
        while *data.get_unchecked(pos) != b',' {
            pos += 1;
        }
        pos += 1;
        
        // Skip "E" field
        while *data.get_unchecked(pos) != b',' {
            pos += 1;
        }
        pos += 1;
        
        // Parse symbol
        parse_symbol_fast(data, &mut pos, &mut trade.symbol, &mut trade.symbol_len);
        pos += 1; // Skip comma
        
        // Skip trade ID field
        while *data.get_unchecked(pos) != b',' {
            pos += 1;
        }
        pos += 1;
        
        // Parse price
        trade.price = parse_number_fast(data, &mut pos);
        pos += 1; // Skip comma
        
        // Parse quantity
        trade.quantity = parse_number_fast(data, &mut pos);
        pos += 1; // Skip comma
        
        // Parse timestamp
        trade.timestamp = parse_timestamp_fast(data, &mut pos);
        pos += 1; // Skip comma
        
        // Parse is_maker
        trade.is_maker = parse_bool_fast(data, &mut pos);
    }
}