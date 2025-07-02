//! # atomic-id
//!
//! A high-performance, thread-safe, and globally unique ID generator.
//! Inspired by Snowflake, it supports multiple bit-widths (24, 32, 64, 128, 256)
//! and encodings, making it suitable for a wide range of applications from database keys
//! to distributed service identifiers.
//!
//! ## Features
//! - **Multiple Bit-Widths**: Generate IDs of 24, 32, 64, 128, or 256 bits.
//! - **Thread-Safe**: Uses atomic operations to guarantee uniqueness across threads.
//! - **Distributed-System Ready**: Incorporates node and shard IDs for global uniqueness.
//! - **Customizable Epoch**: Set a custom epoch for timestamp-based generation.
//! - **Flexible Encodings**: Output IDs in base36, base58, base91, or hexadecimal.
//! - **High Performance**: Optimized for low-latency ID generation in high-throughput systems.
//!
//! ## Feature Flags
//! - `short`: Enables support for 24-bit and 32-bit IDs (`x24`, `x32`). This feature is disabled by default to keep the library lightweight.
//!
//! ## Quick Start
//!
//! Add `atomic-id` to your `Cargo.toml`. To use 24-bit or 32-bit IDs, enable the `short` feature.
//! ```toml
//! [dependencies]
//! atomic-id = { version = "0.1", features = ["short"] } # Replace with the latest version
//! ```
//!
//! ### Generating a 64-bit ID
//! ```rust
//! use atomic_id::{AtomicId, x64};
//!
//! // Generate a new base36-encoded 64-bit ID.
//! let id = AtomicId::<x64>::new();
//! println!("64-bit ID: {}", id);
//! ```
//!
//! ## ID Structure
//!
//! The library generates IDs with different structures depending on the bit-width:
//! - **24-bit**: `24-bit sequence`
//! - **32-bit**: `8-bit thread ID | 24-bit sequence`
//! - **64-bit**: `20-bit timestamp | 12-bit node ID | 8-bit shard ID | 8-bit thread ID | 16-bit sequence`
//! - **128-bit & 256-bit**: More complex structures with higher entropy from timestamps, nanoseconds, and sequences.
//!
//! ## Advanced Usage
//!
//! ### Custom Epoch
//! For timestamp-based IDs (64, 128, 256-bit), you can set a custom epoch.
//! ```rust
//! use atomic_id::AtomicOption;
//! // Set a custom epoch to `2024-01-01 00:00:00 UTC` in milliseconds.
//! AtomicOption::epoch(1704067200000);
//! ```
//!
//! ### Different Encodings
//! ```rust
//! use atomic_id::{AtomicId, x64};
//!
//! let id_base58 = AtomicId::<x64>::base58();
//! let id_hex = AtomicId::<x64>::hex();
//!
//! println!("Base58: {}", id_base58);
//! println!("Hex:    {}", id_hex);
//! ```

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default custom epoch: 2022-05-01 00:00:00 UTC (milliseconds since UNIX_EPOCH)
const DEFAULT_EPOCH: u64 = 1651363200000;

/// Global custom epoch (milliseconds since UNIX_EPOCH).
/// Used as the reference point for all timestamp-based IDs.
/// Can be set/reset via [`AtomicOption`].
static CUSTOM_EPOCH: AtomicU64 = AtomicU64::new(DEFAULT_EPOCH);

/// Global sequence counters for each bit mode.
/// These ensure atomic, thread-safe, and unique sequence numbers for each ID width.
static SEQ_24: AtomicU64 = AtomicU64::new(0);
static SEQ_32: AtomicU64 = AtomicU64::new(0);
static SEQ_64: AtomicU64 = AtomicU64::new(0);
static SEQ_128: AtomicU64 = AtomicU64::new(0);
static SEQ_256: AtomicU64 = AtomicU64::new(0);

// Thread-local storage for thread ID.
// Each thread gets a unique ID (1-128) to add entropy to generated IDs.
thread_local! {
    static THREAD_ID: std::cell::Cell<u8> = std::cell::Cell::new(0);
}

/// The core generator struct for producing unique IDs.
///
/// This struct holds node and shard identifiers, which are incorporated into
/// 64, 128, and 256-bit IDs to ensure uniqueness in a distributed environment.
///
/// While you can create an `IdGenerator` instance, the library is designed
/// to be used through the static methods on [`AtomicId`], which manage a global generator.
pub struct IdGenerator {
    /// Node identifier (0-4095), used in 64, 128, and 256-bit IDs.
    pub node_id: u16,
    /// Shard identifier (0-255), used in 64, 128, and 256-bit IDs.
    pub shard_id: u8,
}

impl IdGenerator {
    /// Create a new generator with the given node and shard IDs.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier (0..=4095).
    /// * `shard_id` - Shard identifier (0..=255).
    ///
    /// # Returns
    /// A new [`IdGenerator`] instance.
    pub fn new(node_id: u16, shard_id: u8) -> Self {
        Self { node_id, shard_id }
    }

    /// Get the current timestamp in milliseconds, relative to the global custom epoch.
    ///
    /// # Returns
    /// Milliseconds since the current epoch (see [`AtomicOption`]).
    fn timestamp(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        now.saturating_sub(CUSTOM_EPOCH.load(Ordering::Relaxed))
    }

    /// Get the current timestamp in nanoseconds since the UNIX epoch.
    ///
    /// # Returns
    /// Nanoseconds since UNIX_EPOCH as a `u64`.
    fn nanos(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    /// Get or assign a unique ID for the current thread.
    ///
    /// This method provides a thread-local ID from 1 to 128, which is used
    /// as a source of entropy in ID generation.
    ///
    /// # Returns
    /// A thread-local unique ID in the range 1..=128.
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

    /// Generate a 24-bit unique ID.
    ///
    /// The ID is generated from a single atomic counter that wraps around.
    /// It is suitable for short-lived, high-throughput scenarios where a small ID is needed.
    ///
    /// - **Structure**: 24 bits for the sequence.
    /// - **Uniqueness**: Guarantees up to 16.7 million (2^24) unique IDs before the counter wraps around.
    ///
    /// # Returns
    /// A 24-bit unique ID as a `u32`.
    pub fn gen24(&self) -> u32 {
        let seq = SEQ_24.fetch_add(1, Ordering::Relaxed);
        (seq & 0xFFFFFF) as u32
    }

    /// Generate a 32-bit unique ID.
    ///
    /// This ID combines a thread-specific identifier with a sequence number,
    /// providing better collision resistance in multi-threaded applications than `gen24`.
    ///
    /// - **Structure**: 8 bits for the thread ID | 24 bits for the sequence.
    ///
    /// # Returns
    /// A 32-bit unique ID as a `u32`.
    pub fn gen32(&self) -> u32 {
        let thread_id = self.thread_id();
        let seq = SEQ_32.fetch_add(1, Ordering::Relaxed);

        let thread_bits = ((thread_id as u32) & 0xFF) << 24;
        let seq_bits = (seq & 0xFFFFFF) as u32;

        thread_bits | seq_bits
    }

    /// Generate a 64-bit unique ID, inspired by Twitter's Snowflake.
    ///
    /// This ID is ideal for distributed systems, as it combines a timestamp,
    /// node/shard identifiers, and a sequence number to ensure global uniqueness.
    ///
    /// - **Structure**: 20-bit timestamp | 12-bit node ID | 8-bit shard ID | 8-bit thread ID | 16-bit sequence.
    /// - **Timestamp**: Milliseconds since the custom epoch, providing a lifespan of ~34 years (2^20 ms).
    /// - **Node ID**: Supports up to 4096 nodes (2^12).
    /// - **Shard ID**: Supports up to 256 shards per node (2^8).
    /// - **Sequence**: Supports up to 65,536 IDs per millisecond per thread (2^16).
    ///
    /// # Returns
    /// A 64-bit unique ID as a `u64`.
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

    /// Generate a 128-bit unique ID with enhanced collision resistance.
    ///
    /// This ID uses a two-part structure to maximize entropy, combining a timestamp-based
    /// part with a high-entropy part derived from nanoseconds and sequence numbers.
    ///
    /// - **High 64 bits**: 32-bit timestamp | 12-bit node | 8-bit shard | 8-bit thread | 4-bit reserved.
    /// - **Low 64 bits**: 32-bit nanoseconds | 24-bit sequence | 8-bit rotated thread ID.
    ///
    /// # Returns
    /// A 128-bit unique ID as a `u128`.
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

    /// Generate a 256-bit unique ID for maximum entropy and uniqueness.
    ///
    /// This ID is constructed from four 64-bit parts, each derived from different
    /// sources of entropy (timestamps, nanoseconds, node/shard/thread IDs, and sequences).
    /// It is suitable for applications requiring cryptographic-level uniqueness.
    ///
    /// # Returns
    /// An array of four `u64` values representing the 256-bit ID.
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

/// Encoding utilities for converting numeric IDs to various string representations.
///
/// Supported encodings:
/// - `base36`: `[0-9a-z]`
/// - `base58`: Bitcoin alphabet (e.g., for short URLs)
/// - `base91`: ASCII-safe and URL-safe
/// - `hex`: `[0-9a-f]`
mod encode {
    /// Bitcoin-style base58 alphabet (no `0`, `O`, `I`, `l`).
    const BASE58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    /// Base91 alphabet (ASCII-safe, URL-safe).
    const BASE91: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~\"";
    /// Base36 alphabet (0-9, a-z).
    const BASE36: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    /// Convert a number to a string in the given base and alphabet.
    ///
    /// # Arguments
    /// * `n` - The number to encode.
    /// * `base` - The target base (e.g., 36, 58, 91).
    /// * `alphabet` - The character set for encoding.
    /// * `min_width` - The minimum width of the output string, padded with the first alphabet character.
    ///
    /// # Returns
    /// The encoded string.
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

    /// Encode a number as a base58 string.
    pub fn base58(n: u128, width: usize) -> String {
        to_base(n, 58, BASE58, width)
    }

    /// Encode a number as a base91 string.
    pub fn base91(n: u128, width: usize) -> String {
        to_base(n, 91, BASE91, width)
    }

    /// Encode a number as a base36 string.
    pub fn base36(n: u128, width: usize) -> String {
        to_base(n, 36, BASE36, width)
    }

    /// Encode a number as a hexadecimal string.
    pub fn hex(n: u128, width: usize) -> String {
        format!("{:0width$x}", n, width = width)
    }
}

/// Global generator instance, initialized on first use.
/// Used by all [`AtomicId`] operations.
static GENERATOR: std::sync::OnceLock<IdGenerator> = std::sync::OnceLock::new();

/// Get a reference to the global [`IdGenerator`] instance.
///
/// Initializes the generator with default values (node_id=1, shard_id=0) on first call.
fn xgen() -> &'static IdGenerator {
    GENERATOR.get_or_init(|| IdGenerator::new(1, 0))
}

// Bit mode constants for compile-time selection.
#[cfg(feature = "short")]
#[allow(non_upper_case_globals)]
/// Constant for 24-bit mode (requires the `short` feature).
pub const x24: usize = 24;
#[cfg(feature = "short")]
#[allow(non_upper_case_globals)]
/// Constant for 32-bit mode (requires the `short` feature).
pub const x32: usize = 32;
#[allow(non_upper_case_globals)]
/// Constant for 64-bit mode.
pub const x64: usize = 64;
#[allow(non_upper_case_globals)]
/// Constant for 128-bit mode.
pub const x128: usize = 128;
#[allow(non_upper_case_globals)]
/// Constant for 256-bit mode.
pub const x256: usize = 256;

/// The main entry point for generating atomic IDs of a specific bit width.
///
/// Use the const generic `BITS` parameter to select the desired ID size.
/// For 24 and 32-bit IDs, the `short` feature must be enabled.
///
/// # Examples
///
/// ```
/// use atomic_id::{AtomicId, x64, x128};
///
/// // Generate a 64-bit ID
/// let id64 = AtomicId::<x64>::new();
///
/// // Generate a 128-bit ID
/// let id128 = AtomicId::<x128>::new();
/// ```
pub struct AtomicId<const BITS: usize>;

#[cfg(feature = "short")]
impl AtomicId<24> {
    /// Generate a new 24-bit ID, encoded as a 5-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x24};
    /// let id = AtomicId::<x24>::new();
    /// assert_eq!(id.len(), 5);
    /// ```
    pub fn new() -> String {
        encode::base36(xgen().gen24() as u128, 5)
    }
    /// Generate a new 24-bit ID, encoded as a 5-character base58 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x24};
    /// let id = AtomicId::<x24>::base58();
    /// assert_eq!(id.len(), 5);
    /// ```
    pub fn base58() -> String {
        encode::base58(xgen().gen24() as u128, 5)
    }
    /// Generate a new 24-bit ID, encoded as a 4-character base91 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x24};
    /// let id = AtomicId::<x24>::base91();
    /// assert_eq!(id.len(), 4);
    /// ```
    pub fn base91() -> String {
        encode::base91(xgen().gen24() as u128, 4)
    }
    /// Generate a new 24-bit ID, encoded as a 5-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x24};
    /// let id = AtomicId::<x24>::base36();
    /// assert_eq!(id.len(), 5);
    /// ```
    pub fn base36() -> String {
        encode::base36(xgen().gen24() as u128, 5)
    }
    /// Generate a new 24-bit ID, encoded as a 6-character hexadecimal string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x24};
    /// let id = AtomicId::<x24>::hex();
    /// assert_eq!(id.len(), 6);
    /// ```
    pub fn hex() -> String {
        encode::hex(xgen().gen24() as u128, 6)
    }

    /// Generate a batch of 24-bit IDs, encoded as base36 strings.
    ///
    /// # Arguments
    /// * `n` - The number of IDs to generate.
    ///
    /// # Returns
    /// A vector of base36-encoded ID strings.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x24};
    /// let ids = AtomicId::<x24>::batch(3);
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    /// Generate a batch of 24-bit IDs as base58 strings.
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    /// Generate a batch of 24-bit IDs as base91 strings.
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    /// Generate a batch of 24-bit IDs as base36 strings.
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    /// Generate a batch of 24-bit IDs as hexadecimal strings.
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

#[cfg(feature = "short")]
impl AtomicId<32> {
    /// Generate a new 32-bit ID, encoded as a 7-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x32};
    /// let id = AtomicId::<x32>::new();
    /// assert_eq!(id.len(), 7);
    /// ```
    pub fn new() -> String {
        encode::base36(xgen().gen32() as u128, 7)
    }
    /// Generate a new 32-bit ID, encoded as a 6-character base58 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x32};
    /// let id = AtomicId::<x32>::base58();
    /// assert_eq!(id.len(), 6);
    /// ```
    pub fn base58() -> String {
        encode::base58(xgen().gen32() as u128, 6)
    }
    /// Generate a new 32-bit ID, encoded as a 5-character base91 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x32};
    /// let id = AtomicId::<x32>::base91();
    /// assert_eq!(id.len(), 5);
    /// ```
    pub fn base91() -> String {
        encode::base91(xgen().gen32() as u128, 5)
    }
    /// Generate a new 32-bit ID, encoded as a 7-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x32};
    /// let id = AtomicId::<x32>::base36();
    /// assert_eq!(id.len(), 7);
    /// ```
    pub fn base36() -> String {
        encode::base36(xgen().gen32() as u128, 7)
    }
    /// Generate a new 32-bit ID, encoded as an 8-character hexadecimal string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x32};
    /// let id = AtomicId::<x32>::hex();
    /// assert_eq!(id.len(), 8);
    /// ```
    pub fn hex() -> String {
        encode::hex(xgen().gen32() as u128, 8)
    }

    /// Generate a batch of 32-bit IDs, encoded as base36 strings.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x32};
    /// let ids = AtomicId::<x32>::batch(3);
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    /// Generate a batch of 32-bit IDs as base58 strings.
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    /// Generate a batch of 32-bit IDs as base91 strings.
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    /// Generate a batch of 32-bit IDs as base36 strings.
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    /// Generate a batch of 32-bit IDs as hexadecimal strings.
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

impl AtomicId<64> {
    /// Generate a new 64-bit ID, encoded as a 13-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let id = AtomicId::<x64>::new();
    /// assert_eq!(id.len(), 13);
    /// ```
    pub fn new() -> String {
        encode::base36(xgen().gen64() as u128, 13)
    }
    /// Generate a new 64-bit ID, encoded as an 11-character base58 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let id = AtomicId::<x64>::base58();
    /// assert_eq!(id.len(), 11);
    /// ```
    pub fn base58() -> String {
        encode::base58(xgen().gen64() as u128, 11)
    }
    /// Generate a new 64-bit ID, encoded as a 10-character base91 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let id = AtomicId::<x64>::base91();
    /// assert_eq!(id.len(), 10);
    /// ```
    pub fn base91() -> String {
        encode::base91(xgen().gen64() as u128, 10)
    }
    /// Generate a new 64-bit ID, encoded as a 13-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let id = AtomicId::<x64>::base36();
    /// assert_eq!(id.len(), 13);
    /// ```
    pub fn base36() -> String {
        encode::base36(xgen().gen64() as u128, 13)
    }
    /// Generate a new 64-bit ID, encoded as a 16-character hexadecimal string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let id = AtomicId::<x64>::hex();
    /// assert_eq!(id.len(), 16);
    /// ```
    pub fn hex() -> String {
        encode::hex(xgen().gen64() as u128, 16)
    }

    /// Generate a batch of 64-bit IDs, encoded as base36 strings.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let ids = AtomicId::<x64>::batch(3);
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    /// Generate a batch of 64-bit IDs as base58 strings.
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    /// Generate a batch of 64-bit IDs as base91 strings.
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    /// Generate a batch of 64-bit IDs as base36 strings.
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    /// Generate a batch of 64-bit IDs as hexadecimal strings.
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }

    /// Generate a sequential 64-bit ID as a base36 string.
    ///
    /// This method uses a simple atomic counter, making the IDs sequential but not
    /// time-sortable. It is useful for scenarios where strict ordering is more
    /// important than distributed uniqueness.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x64};
    /// let id1 = AtomicId::<x64>::sequential();
    /// let id2 = AtomicId::<x64>::sequential();
    /// // id2 will be lexicographically greater than id1
    /// assert!(id2 > id1);
    /// ```
    pub fn sequential() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base36(seq as u128, 13)
    }

    /// Generate a sequential 64-bit ID as a base58 string.
    pub fn sequential_base58() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base58(seq as u128, 11)
    }

    /// Generate a sequential 64-bit ID as a base91 string.
    pub fn sequential_base91() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base91(seq as u128, 10)
    }

    /// Generate a sequential 64-bit ID as a base36 string.
    pub fn sequential_base36() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::base36(seq as u128, 13)
    }

    /// Generate a sequential 64-bit ID as a hexadecimal string.
    pub fn sequential_hex() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        encode::hex(seq as u128, 16)
    }

    /// Generate a batch of sequential 64-bit IDs as base36 strings.
    pub fn sequential_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential()).collect()
    }
    /// Generate a batch of sequential 64-bit IDs as base58 strings.
    pub fn sequential_base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_base58()).collect()
    }
    /// Generate a batch of sequential 64-bit IDs as base91 strings.
    pub fn sequential_base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_base91()).collect()
    }
    /// Generate a batch of sequential 64-bit IDs as base36 strings.
    pub fn sequential_base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_base36()).collect()
    }
    /// Generate a batch of sequential 64-bit IDs as hexadecimal strings.
    pub fn sequential_hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::sequential_hex()).collect()
    }
}

impl AtomicId<128> {
    /// Generate a new 128-bit ID, encoded as a 25-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x128};
    /// let id = AtomicId::<x128>::new();
    /// assert_eq!(id.len(), 25);
    /// ```
    pub fn new() -> String {
        encode::base36(xgen().gen128(), 25)
    }
    /// Generate a new 128-bit ID, encoded as a 22-character base58 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x128};
    /// let id = AtomicId::<x128>::base58();
    /// assert_eq!(id.len(), 22);
    /// ```
    pub fn base58() -> String {
        encode::base58(xgen().gen128(), 22)
    }
    /// Generate a new 128-bit ID, encoded as a 20-character base91 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x128};
    /// let id = AtomicId::<x128>::base91();
    /// assert_eq!(id.len(), 20);
    /// ```
    pub fn base91() -> String {
        encode::base91(xgen().gen128(), 20)
    }
    /// Generate a new 128-bit ID, encoded as a 25-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x128};
    /// let id = AtomicId::<x128>::base36();
    /// assert_eq!(id.len(), 25);
    /// ```
    pub fn base36() -> String {
        encode::base36(xgen().gen128(), 25)
    }
    /// Generate a new 128-bit ID, encoded as a 32-character hexadecimal string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x128};
    /// let id = AtomicId::<x128>::hex();
    /// assert_eq!(id.len(), 32);
    /// ```
    pub fn hex() -> String {
        encode::hex(xgen().gen128(), 32)
    }

    /// Generate a batch of 128-bit IDs, encoded as base36 strings.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x128};
    /// let ids = AtomicId::<x128>::batch(3);
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    /// Generate a batch of 128-bit IDs as base58 strings.
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    /// Generate a batch of 128-bit IDs as base91 strings.
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    /// Generate a batch of 128-bit IDs as base36 strings.
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    /// Generate a batch of 128-bit IDs as hexadecimal strings.
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

impl AtomicId<256> {
    /// Generate a new 256-bit ID, encoded as a 52-character base36 string.
    ///
    /// The raw 256-bit ID is composed of four 64-bit parts, each of which is
    /// encoded into a 13-character base36 string and then joined.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x256};
    /// let id = AtomicId::<x256>::new();
    /// assert_eq!(id.len(), 52);
    /// ```
    pub fn new() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base36(p as u128, 13))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Generate a new 256-bit ID, encoded as a 44-character base58 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x256};
    /// let id = AtomicId::<x256>::base58();
    /// assert_eq!(id.len(), 44);
    /// ```
    pub fn base58() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base58(p as u128, 11))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Generate a new 256-bit ID, encoded as a 40-character base91 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x256};
    /// let id = AtomicId::<x256>::base91();
    /// assert_eq!(id.len(), 40);
    /// ```
    pub fn base91() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base91(p as u128, 10))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Generate a new 256-bit ID, encoded as a 52-character base36 string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x256};
    /// let id = AtomicId::<x256>::base36();
    /// assert_eq!(id.len(), 52);
    /// ```
    pub fn base36() -> String {
        let parts = xgen().gen256();
        parts
            .iter()
            .map(|&p| encode::base36(p as u128, 13))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Generate a new 256-bit ID, encoded as a 64-character hexadecimal string.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x256};
    /// let id = AtomicId::<x256>::hex();
    /// assert_eq!(id.len(), 64);
    /// ```
    pub fn hex() -> String {
        let parts = xgen().gen256();
        format!(
            "{:016x}{:016x}{:016x}{:016x}",
            parts[0], parts[1], parts[2], parts[3]
        )
    }

    /// Generate a batch of 256-bit IDs, encoded as base36 strings.
    ///
    /// # Example
    /// ```
    /// use atomic_id::{AtomicId, x256};
    /// let ids = AtomicId::<x256>::batch(3);
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::new()).collect()
    }
    /// Generate a batch of 256-bit IDs as base58 strings.
    pub fn base58_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base58()).collect()
    }
    /// Generate a batch of 256-bit IDs as base91 strings.
    pub fn base91_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base91()).collect()
    }
    /// Generate a batch of 256-bit IDs as base36 strings.
    pub fn base36_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::base36()).collect()
    }
    /// Generate a batch of 256-bit IDs as hexadecimal strings.
    pub fn hex_batch(n: usize) -> Vec<String> {
        (0..n).map(|_| Self::hex()).collect()
    }
}

/// Provides methods for configuring global settings for `atomic-id`.
///
/// Use this struct to manage the global epoch for timestamp-based ID generation.
pub struct AtomicOption;

impl AtomicOption {
    /// Set the global custom epoch for timestamp-based IDs.
    ///
    /// The epoch is the point in time from which the timestamp portion of an ID is measured.
    /// Setting a more recent epoch can extend the lifespan of the generator.
    ///
    /// # Arguments
    /// * `ms` - The epoch timestamp in milliseconds since the UNIX epoch.
    ///
    /// # Example
    /// ```
    /// use atomic_id::AtomicOption;
    /// // Set the epoch to January 1, 2024
    /// AtomicOption::epoch(1704067200000);
    /// ```
    pub fn epoch(ms: u64) {
        CUSTOM_EPOCH.store(ms, Ordering::Relaxed);
    }

    /// Get the current global epoch value.
    ///
    /// # Returns
    /// The current epoch in milliseconds since the UNIX epoch.
    pub fn get_epoch() -> u64 {
        CUSTOM_EPOCH.load(Ordering::Relaxed)
    }

    /// Reset the global epoch to its default value (`2022-05-01 00:00:00 UTC`).
    pub fn reset_epoch() {
        CUSTOM_EPOCH.store(DEFAULT_EPOCH, Ordering::Relaxed);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Test generation of 64-bit IDs in all encodings.
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

    /// Test generation of 128-bit IDs.
    #[test]
    fn test_gen128() {
        let id = AtomicId::<128>::base36();
        println!("Generated ID: {}", id);
    }

    /// Test generation of 256-bit IDs.
    #[test]
    fn test_gen256() {
        let id = AtomicId::<256>::base36();
        println!("Generated ID: {}", id);
    }

    /// Test uniqueness of 64-bit IDs over 10 million generations.
    #[test]
    #[ignore] // This test is long-running and should be run manually.
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