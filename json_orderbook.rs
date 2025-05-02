#[derive(Debug, Copy, Clone)]
pub struct Level {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug)]
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
unsafe fn parse_number_fast(bytes: &[u8], pos: &mut usize) -> f64 {
    let mut int_part = 0u64;
    let mut frac_part = 0u64;
    let mut frac_digits = 0u32;
    
    // Skip until we find the first digit
    while *bytes.get_unchecked(*pos) == b'"' || *bytes.get_unchecked(*pos) == b'[' {
        *pos += 1;
    }
    
    // Integer part using bitwise operations
    while *bytes.get_unchecked(*pos) != b'.' {
        int_part = (int_part << 3) + (int_part << 1) + // x * 10 = x * (8 + 2)
            (*bytes.get_unchecked(*pos) & 0x0f) as u64; // faster than - b'0'
        *pos += 1;
    }
    *pos += 1; // skip decimal
    
    // Fractional part
    while *bytes.get_unchecked(*pos) != b'"' {
        frac_part = (frac_part << 3) + (frac_part << 1) +
            (*bytes.get_unchecked(*pos) & 0x0f) as u64;
        frac_digits += 1;
        *pos += 1;
    }
    
    *pos += 1; // skip closing quote
    
    // Combine parts using a lookup table for power of 10
    const POW10: [f64; 9] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8];
    int_part as f64 + (frac_part as f64 / POW10[frac_digits as usize])
}

#[inline(always)]
pub fn parse_orderbook<const DEPTH: usize>(data: &[u8], book: &mut OrderBook<DEPTH>) {
    let mut pos = 0;
    
    // Fast forward to bids array using SIMD-like batch checking
    while pos + 8 < data.len() {
        if data[pos] == b'[' && data[pos + 1] == b'[' {
            break;
        }
        pos += 1;
    }
    pos += 1; // position at first bid level
    
    // Parse bids
    unsafe {
        for i in 0..DEPTH {
            pos += 1; // skip [
            let price = parse_number_fast(data, &mut pos);
            pos += 1; // skip comma
            let quantity = parse_number_fast(data, &mut pos);
            pos += 2; // skip ],
            *book.bids.get_unchecked_mut(i) = Level { price, quantity };
        }
        
        // Skip to asks array using SIMD-like batch checking
        while pos + 8 < data.len() {
            if data[pos] == b'[' && data[pos + 1] == b'[' {
                break;
            }
            pos += 1;
        }
        pos += 1;
        
        // Parse asks
        for i in 0..DEPTH {
            pos += 1; // skip [
            let price = parse_number_fast(data, &mut pos);
            pos += 1; // skip comma
            let quantity = parse_number_fast(data, &mut pos);
            pos += 2; // skip ],
            *book.asks.get_unchecked_mut(i) = Level { price, quantity };
        }
    }
}
