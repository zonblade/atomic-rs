use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default custom epoch: 2022-05-01 00:00:00 UTC
const DEFAULT_EPOCH: u64 = 1651363200000;

/// Global custom epoch
static CUSTOM_EPOCH: AtomicU64 = AtomicU64::new(DEFAULT_EPOCH);

/// Global sequence counters for each bit mode
static SEQ_24: AtomicU64 = AtomicU64::new(0);
static SEQ_32: AtomicU64 = AtomicU64::new(0);
static SEQ_64: AtomicU64 = AtomicU64::new(0);
static SEQ_128: AtomicU64 = AtomicU64::new(0);
static SEQ_256: AtomicU64 = AtomicU64::new(0);

// Thread-local storage for thread ID
thread_local! {
    static THREAD_ID: std::cell::Cell<u8> = std::cell::Cell::new(0);
}

/// Enhanced ObjectId-style ID Generator
pub struct IdGenerator {
    node_id: u16,
    shard_id: u8,
}

impl IdGenerator {
    /// Create a new generator
    pub fn new(node_id: u16, shard_id: u8) -> Self {
        Self { node_id, shard_id }
    }

    /// Get current timestamp relative to epoch
    fn timestamp(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        now.saturating_sub(CUSTOM_EPOCH.load(Ordering::Relaxed))
    }

    /// Get nanosecond precision timestamp
    fn nanos(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    /// Get or assign thread ID
    fn thread_id(&self) -> u8 {
        THREAD_ID.with(|id| {
            let tid = id.get();
            if tid == 0 {
                let mut hasher = DefaultHasher::new();
                std::thread::current().id().hash(&mut hasher);
                let new_id = ((hasher.finish() & 0x7F) as u8) + 1; // 1-128
                id.set(new_id);
                new_id
            } else {
                tid
            }
        })
    }

    /// Generate 24-bit ID with maximum uniqueness
    /// Pure counter-based for 16.7M unique values
    pub fn gen24(&self) -> u32 {
        let seq = SEQ_24.fetch_add(1, Ordering::Relaxed);
        (seq & 0xFFFFFF) as u32
    }

    /// Generate 32-bit ID
    /// 8-bit thread | 24-bit sequence for better uniqueness
    pub fn gen32(&self) -> u32 {
        let thread_id = self.thread_id();
        let seq = SEQ_32.fetch_add(1, Ordering::Relaxed);

        let thread_bits = ((thread_id as u32) & 0xFF) << 24;
        let seq_bits = (seq & 0xFFFFFF) as u32;

        thread_bits | seq_bits
    }

    /// Generate 64-bit ID with proper ObjectId structure
    /// 20-bit timestamp | 12-bit node | 8-bit shard | 8-bit thread | 16-bit sequence
    pub fn gen64(&self) -> u64 {
        let ts = self.timestamp();
        let thread_id = self.thread_id();
        let seq = SEQ_64.fetch_add(1, Ordering::Relaxed);

        let ts_bits = (ts & 0xFFFFF) << 44;
        let node_bits = ((self.node_id & 0xFFF) as u64) << 32;
        let shard_bits = ((self.shard_id as u64) & 0xFF) << 24;
        let thread_bits = ((thread_id as u64) & 0xFF) << 16;
        let seq_bits = seq & 0xFFFF;

        ts_bits | node_bits | shard_bits | thread_bits | seq_bits
    }

    /// Generate 128-bit ID with enhanced collision resistance
    pub fn gen128(&self) -> u128 {
        let ts = self.timestamp();
        let thread_id = self.thread_id();
        let nanos = self.nanos();
        let seq = SEQ_128.fetch_add(1, Ordering::Relaxed);

        // First 64 bits: Enhanced timestamp-based entropy
        // 32-bit timestamp | 12-bit node | 8-bit shard | 8-bit thread | 4-bit reserved
        let high_part = {
            let ts_bits = (ts & 0xFFFFFFFF) << 32;
            let node_bits = ((self.node_id & 0xFFF) as u64) << 20;
            let shard_bits = ((self.shard_id as u64) & 0xFF) << 12;
            let thread_bits = ((thread_id as u64) & 0xFF) << 4;
            let reserved = (nanos.rotate_right(16)) & 0xF;
            
            ts_bits | node_bits | shard_bits | thread_bits | reserved
        };

        // Second 64 bits: Maximum entropy mixing
        // 32-bit nanos | 24-bit sequence | 8-bit thread rotated
        let low_part = {
            let nanos_bits = (nanos & 0xFFFFFFFF) << 32;
            let seq_bits = (seq & 0xFFFFFF) << 8;
            let thread_rot = thread_id.rotate_left(3) as u64;
            
            nanos_bits | seq_bits | thread_rot
        };

        ((high_part as u128) << 64) | (low_part as u128)
    }

    /// Generate 256-bit ID with maximum entropy
    pub fn gen256(&self) -> [u64; 4] {
        let ts = self.timestamp();
        let thread_id = self.thread_id();
        let nanos = self.nanos();
        let seq = SEQ_256.fetch_add(1, Ordering::Relaxed);

        // Part 0: Base 64-bit structure (like gen64 but with different sequence)
        let part0 = {
            let ts_bits = (ts & 0xFFFFF) << 44;
            let node_bits = ((self.node_id & 0xFFF) as u64) << 32;
            let shard_bits = ((self.shard_id as u64) & 0xFF) << 24;
            let thread_bits = ((thread_id as u64) & 0xFF) << 16;
            let seq_bits = seq & 0xFFFF;
            
            ts_bits | node_bits | shard_bits | thread_bits | seq_bits
        };

        // Part 1: Nanosecond precision with entropy mixing
        let part1 = {
            let nanos_high = (nanos >> 32) & 0xFFFFFFFF;
            let nanos_low = nanos & 0xFFFFFFFF;
            let mixed = nanos_high.rotate_left(16) ^ nanos_low;
            
            (mixed << 32) | ((seq.rotate_right(8)) & 0xFFFFFFFF)
        };

        // Part 2: Thread and sequence entropy
        let part2 = {
            let thread_expanded = ((thread_id as u64) << 56) | 
                                ((thread_id as u64).rotate_left(8) << 48) |
                                ((thread_id as u64).rotate_left(16) << 40) |
                                ((thread_id as u64).rotate_left(24) << 32);
            let seq_mixed = (seq.rotate_left(16)) & 0xFFFFFFFF;
            
            thread_expanded | seq_mixed
        };

        // Part 3: Additional entropy sources
        let part3 = {
            let ts_rotated = ts.rotate_right(12);
            let node_expanded = ((self.node_id as u64) << 48) | 
                              ((self.node_id as u64).rotate_left(4) << 32);
            let shard_expanded = ((self.shard_id as u64) << 24) |
                               ((self.shard_id as u64).rotate_left(2) << 16) |
                               ((self.shard_id as u64).rotate_left(4) << 8) |
                               (self.shard_id as u64).rotate_left(6);
            let mixed_entropy = (ts_rotated & 0xFFFF) | node_expanded | shard_expanded;
            
            mixed_entropy
        };

        [part0, part1, part2, part3]
    }
}

/// Encoding utilities
mod encode {
    const BASE58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    const BASE91: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~\"";
    const BASE36: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    pub fn to_base(mut n: u128, base: usize, alphabet: &[u8], min_width: usize) -> String {
        if n == 0 {
            return String::from_utf8(vec![alphabet[0]; min_width]).unwrap();
        }

        let mut chars = Vec::with_capacity(min_width);
        while n > 0 {
            chars.push(alphabet[(n % base as u128) as usize]);
            n /= base as u128;
        }

        while chars.len() < min_width {
            chars.push(alphabet[0]);
        }

        chars.reverse();
        String::from_utf8(chars).unwrap()
    }

    pub fn base58(n: u128, width: usize) -> String {
        to_base(n, 58, BASE58, width)
    }

    pub fn base91(n: u128, width: usize) -> String {
        to_base(n, 91, BASE91, width)
    }

    pub fn base36(n: u128, width: usize) -> String {
        to_base(n, 36, BASE36, width)
    }

    pub fn hex(n: u128, width: usize) -> String {
        format!("{:0width$x}", n, width = width)
    }
}

/// Global generator instance
static GENERATOR: std::sync::OnceLock<IdGenerator> = std::sync::OnceLock::new();

fn xgen() -> &'static IdGenerator {
    GENERATOR.get_or_init(|| IdGenerator::new(1, 0))
}

// Bit mode constants
#[cfg(feature = "short")]
#[allow(non_upper_case_globals)]
pub const x24: usize = 24;
#[cfg(feature = "short")]
#[allow(non_upper_case_globals)]
pub const x32: usize = 32;
#[allow(non_upper_case_globals)]
pub const x64: usize = 64;
#[allow(non_upper_case_globals)]
pub const x128: usize = 128;
#[allow(non_upper_case_globals)]
pub const x256: usize = 256;

/// Bit mode implementations
pub struct AtomicId<const BITS: usize>;

#[cfg(feature = "short")]
impl AtomicId<24> {
    pub fn new() -> String {
        encode::base36(xgen().gen24() as u128, 5)
    }
    pub fn base58() -> String {
        encode::base58(xgen().gen24() as u128, 5)
    }
    pub fn base91() -> String {
        encode::base91(xgen().gen24() as u128, 4)
    }
    pub fn base36() -> String {
        encode::base36(xgen().gen24() as u128, 5)
    }
    pub fn hex() -> String {
        encode::hex(xgen().gen24() as u128, 6)
    }

    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

#[cfg(feature = "short")]
impl AtomicId<32> {
    pub fn new() -> String {
        encode::base36(xgen().gen32() as u128, 7)
    }
    pub fn base58() -> String {
        encode::base58(xgen().gen32() as u128, 6)
    }
    pub fn base91() -> String {
        encode::base91(xgen().gen32() as u128, 5)
    }
    pub fn base36() -> String {
        encode::base36(xgen().gen32() as u128, 7)
    }
    pub fn hex() -> String {
        encode::hex(xgen().gen32() as u128, 8)
    }

    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

impl AtomicId<64> {
    pub fn new() -> String {
        encode::base36(xgen().gen64() as u128, 13)
    }
    pub fn base58() -> String {
        encode::base58(xgen().gen64() as u128, 11)
    }
    pub fn base91() -> String {
        encode::base91(xgen().gen64() as u128, 10)
    }
    pub fn base36() -> String {
        encode::base36(xgen().gen64() as u128, 13)
    }
    pub fn hex() -> String {
        encode::hex(xgen().gen64() as u128, 16)
    }

    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }

    // Sequential mode for maximum uniqueness
    pub fn sequential() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base36(seq as u128, 13)
    }

    pub fn sequential_base58() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base58(seq as u128, 11)
    }

    pub fn sequential_base91() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base91(seq as u128, 10)
    }

    pub fn sequential_base36() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base36(seq as u128, 13)
    }

    pub fn sequential_hex() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::hex(seq as u128, 16)
    }

    pub fn sequential_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential()).collect()
    }
    pub fn sequential_base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_base58()).collect()
    }
    pub fn sequential_base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_base91()).collect()
    }
    pub fn sequential_base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_base36()).collect()
    }
    pub fn sequential_hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_hex()).collect()
    }
}

impl AtomicId<128> {
    pub fn new() -> String {
        encode::base36(xgen().gen128(), 25)
    }
    pub fn base58() -> String {
        encode::base58(xgen().gen128(), 22)
    }
    pub fn base91() -> String {
        encode::base91(xgen().gen128(), 20)
    }
    pub fn base36() -> String {
        encode::base36(xgen().gen128(), 25)
    }
    pub fn hex() -> String {
        encode::hex(xgen().gen128(), 32)
    }

    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

impl AtomicId<256> {
    pub fn new() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base36(p as u128, 13))
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn base58() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base58(p as u128, 11))
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn base91() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base91(p as u128, 10))
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn base36() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base36(p as u128, 13))
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn hex() -> String {
        let parts = xgen().gen256();
        format!(
            "{:016x}{:016x}{:016x}{:016x}",
            parts[0], parts[1], parts[2], parts[3]
        )
    }

    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

/// Global AtomicId API with epoch management
pub struct AtomicOption;

impl AtomicOption {
    /// Set global epoch in milliseconds
    pub fn epoch(ms: u64) {
        CUSTOM_EPOCH.store(ms, Ordering::Relaxed);
    }

    /// Get current epoch value
    pub fn get_epoch() -> u64 {
        CUSTOM_EPOCH.load(Ordering::Relaxed)
    }

    /// Reset epoch to default value
    pub fn reset_epoch() {
        CUSTOM_EPOCH.store(DEFAULT_EPOCH, Ordering::Relaxed);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen64() {
        let id = AtomicId::<64>::new();
        println!("Generated ID: {}", id);
        let id = AtomicId::<64>::base36();
        println!("Generated ID: {}", id);
        let id = AtomicId::<64>::base58();
        println!("Generated ID: {}", id);
        let id = AtomicId::<64>::base91();
        println!("Generated ID: {}", id);
        let id = AtomicId::<64>::hex();
        println!("Generated ID: {}", id);
    }

    #[test]
    fn test_gen128() {
        let id = AtomicId::<128>::base36();
        println!("Generated ID: {}", id);
    }

    #[test]
    fn test_gen256() {
        let id = AtomicId::<256>::base36();
        println!("Generated ID: {}", id);
    }

    #[test]
    fn test_uniqueness_64(){
        // test uniqueness of 64-bit IDs
        let mut ids = std::collections::HashSet::new();
        for _ in 0..10_000_000 {
            let id = AtomicId::<64>::new();
            assert!(ids.insert(id), "Duplicate ID found");  
        }
        println!("All IDs are unique");
    }
}