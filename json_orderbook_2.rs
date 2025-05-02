#[derive(Debug, Copy, Clone)]
pub struct Level {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug, Clone)]  // Add Clone for OrderBook
pub struct OrderBook<const DEPTH: usize> {
    pub bids: [Level; DEPTH],
    pub asks: [Level; DEPTH],
}

impl<const DEPTH: usize> OrderBook<DEPTH> {
    #[inline(always)]
    pub fn new() -> Self {
        OrderBook {
            bids: [Level { price: 0.0, quantity: 0.0 }; DEPTH],
            asks: [Level { price: 0.0, quantity: 0.0 }; DEPTH],
        }
    }
}

// Optimized number parsing using bitwise operations
#[inline(always)]
fn parse_number_fast(bytes: &[u8], pos: &mut usize) -> Option<f64> {
    if *pos >= bytes.len() {
        return None;
    }

    let mut int_part = 0u64;
    let mut frac_part = 0u64;
    let mut frac_digits = 0u32;
    
    // Skip quotes and brackets safely
    while *pos < bytes.len() && (bytes[*pos] == b'"' || bytes[*pos] == b'[') {
        *pos += 1;
    }

    // Check if we've reached the end
    if *pos >= bytes.len() {
        return None;
    }

    // Parse integer part
    while *pos < bytes.len() && bytes[*pos] != b'.' {
        let digit = bytes[*pos];
        if !digit.is_ascii_digit() {
            return None;
        }
        int_part = (int_part << 3) + (int_part << 1) + (digit & 0x0f) as u64;
        *pos += 1;
    }

    // Skip decimal point
    if *pos >= bytes.len() || bytes[*pos] != b'.' {
        return None;
    }
    *pos += 1;

    // Parse fractional part
    while *pos < bytes.len() && bytes[*pos] != b'"' {
        let digit = bytes[*pos];
        if !digit.is_ascii_digit() {
            return None;
        }
        frac_part = (frac_part << 3) + (frac_part << 1) + (digit & 0x0f) as u64;
        frac_digits += 1;
        *pos += 1;
    }

    // Skip closing quote
    if *pos < bytes.len() && bytes[*pos] == b'"' {
        *pos += 1;
    }

    const POW10: [f64; 9] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8];
    
    if frac_digits as usize >= POW10.len() {
        return None;
    }

    Some(int_part as f64 + (frac_part as f64 / POW10[frac_digits as usize]))
}

#[inline(always)]
pub fn parse_orderbook<const DEPTH: usize>(data: &[u8], book: &mut OrderBook<DEPTH>) -> Option<()> {
    let mut pos = 0;
    
    // Fast forward to bids array
    while pos + 1 < data.len() {
        if data[pos] == b'[' && data[pos + 1] == b'[' {
            break;
        }
        pos += 1;
    }
    
    if pos + 1 >= data.len() {
        return None;
    }
    
    pos += 1; // position at first bid level

    // Parse bids
    for i in 0..DEPTH {
        if pos >= data.len() {
            return None;
        }
        
        pos += 1; // skip [
        let price = parse_number_fast(data, &mut pos)?;
        pos += 1; // skip comma
        let quantity = parse_number_fast(data, &mut pos)?;
        pos += 2; // skip ],
        
        book.bids[i] = Level { price, quantity };
    }

    // Skip to asks array
    while pos + 1 < data.len() {
        if data[pos] == b'[' && data[pos + 1] == b'[' {
            break;
        }
        pos += 1;
    }
    
    if pos + 1 >= data.len() {
        return None;
    }
    
    pos += 1;

    // Parse asks
    for i in 0..DEPTH {
        if pos >= data.len() {
            return None;
        }
        
        pos += 1; // skip [
        let price = parse_number_fast(data, &mut pos)?;
        pos += 1; // skip comma
        let quantity = parse_number_fast(data, &mut pos)?;
        pos += 2; // skip ],
        
        book.asks[i] = Level { price, quantity };
    }

    Some(())
}