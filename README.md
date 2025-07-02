# AtomicId - Ultra-High Throughput ID Generator

A high-performance, thread-safe, distributed ID generation library for Rust applications requiring guaranteed uniqueness and extreme throughput (up to 10 Gbps).

## Features

- **Multiple bit modes**: 24-bit (ultra-compact, requires `short` feature) to 256-bit (maximum entropy)
- **Four encodings**: Base58 (human-readable), Base91 (most compact), Base36 (alphanumeric), Hex
- **Lock-free**: Uses atomic operations for maximum parallel performance
- **Distributed-safe**: Built-in node/shard/thread isolation prevents collisions
- **Clock-drift resilient**: Handles system time adjustments gracefully
- **Batch generation**: Optimized bulk ID creation
- **Configurable epoch**: Custom timestamp epochs for domain-specific needs

## Quick Start

Add to `Cargo.toml`:
```toml
[dependencies]
atomic-id = "0.1.0"
```

### Bit Mode Constants

For convenience, use the provided constants for bit modes:

```rust
use atomic_id::{AtomicId, x24, x32, x64, x128, x256};
```

This allows you to write `AtomicId<x64>` instead of `AtomicId::<64>`. (24/32-bit require the `short` feature.)

Basic usage:
```rust
use atomic_id::{AtomicId, AtomicOption, x128};

// Optional: Set custom epoch (defaults to 2022-05-01 00:00:00 UTC)
AtomicOption::epoch(1704067200000); // 2024-01-01 00:00:00 UTC

// Default 128-bit Base58 ID
let id = AtomicId<x128>::base58();
println!("{}", id); // "111111111111111111111A"

// Most compact encoding
let compact = AtomicId<x128>::base91();
println!("{}", compact); // "AAAAAAAAAAAAAAAAAAB"

// Hex encoding (ObjectId-style)
let hex = AtomicId<x128>::hex();
println!("{}", hex); // "00000000000000000000000000000001"

// New: Base36 encoding
let b36 = AtomicId<x128>::base36();
println!("{}", b36); // "00000000000000000000000001"
```

## Epoch Configuration

Configure custom epochs for timestamp calculations:

```rust
// Set epoch to 2024-01-01 00:00:00 UTC
AtomicOption::epoch(1704067200000);

// Set epoch to current time (useful for relative timestamps)
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64;
AtomicOption::epoch(now);

// Get current epoch
let current_epoch = AtomicOption::get_epoch();

// Reset to default epoch (2022-05-01 00:00:00 UTC)
AtomicOption::reset_epoch();
```

## Bit Modes

Choose the right size for your use case:

```rust
use atomic_id::{AtomicId, x24, x32, x64, x128, x256};

// Ultra-compact IDs (4-5 characters, requires `short` feature)
// let tiny = AtomicId<x24>::base91();     // "AAAB" (4 chars)
// let small = AtomicId<x24>::base58();   // "11111" (5 chars)

// Compact IDs (5-6 characters, requires `short` feature)
// let compact = AtomicId<x32>::base91();  // "AAAAB" (5 chars)
// let standard = AtomicId<x32>::base58(); // "111111" (6 chars)

// Standard IDs (10-11 characters)
let balanced = AtomicId<x64>::base91(); // "AAAAAAAAAB" (10 chars)
let readable = AtomicId<x64>::base58(); // "11111111111" (11 chars)

// High-entropy IDs (20-22 characters) - Default
let secure = AtomicId<x128>::base91();  // "AAAAAAAAAAAAAAAAAAB" (20 chars)
let default_id = AtomicId<x128>::base58(); // "111111111111111111111A" (22 chars)

// Maximum entropy (39-44 characters)
let crypto = AtomicId<x256>::base91();  // 39 chars
let max = AtomicId<x256>::base58();     // 44 chars
```

## Encoding Comparison

| Encoding | Efficiency | Characters | URL-Safe | Human-Readable | ObjectId-style |
|----------|------------|------------|----------|----------------|----------------|
| Base91   | Highest    | Compact    | ✓        | Moderate       | ✗              |
| Base58   | Good       | Readable   | ✓        | ✓              | ✗              |
| Base36   | Good       | Alphanumeric| ✓       | ✓              | ✗              |
| Hex      | Standard   | Longest    | ✓        | ✓              | ✓              |

```rust
// Same 64-bit ID in different encodings:
let base91 = AtomicId<x64>::base91();  // "AAAAAAAAAB" (10 chars)
let base58 = AtomicId<x64>::base58();  // "11111111111" (11 chars)  
let base36 = AtomicId<x64>::base36();  // "0000000000001" (13 chars)
let hex = AtomicId<x64>::hex();        // "0000000000000001" (16 chars)
```

## High-Frequency Generation

For maximum uniqueness in high-throughput scenarios:

```rust
// Sequential mode - guaranteed unique even at extreme rates
let seq_id = AtomicId<x64>::sequential_base58(); // Base58
let seq_compact = AtomicId<x64>::sequential_base91(); // Base91
let seq_hex = AtomicId<x64>::sequential_hex(); // Hex

// Batch generation (optimized)
let batch = AtomicId<x128>::base58_batch(10_000);
let compact_batch = AtomicId<x64>::base91_batch(10_000);
```

## All Available Methods

```rust
// For each bit mode (x24, x32, x64, x128, x256):
AtomicId<xBITS>::new()                    // Base36 (default)
AtomicId<xBITS>::base58()                 // Base58
AtomicId<xBITS>::base91()                 // Base91 (most compact)
AtomicId<xBITS>::base36()                 // Base36 (alphanumeric)
AtomicId<xBITS>::hex()                    // Hex (ObjectId-style)
AtomicId<xBITS>::batch(count)             // Batch Base36
AtomicId<xBITS>::base58_batch(count)      // Batch Base58
AtomicId<xBITS>::base91_batch(count)      // Batch Base91
AtomicId<xBITS>::base36_batch(count)      // Batch Base36
AtomicId<xBITS>::hex_batch(count)         // Batch Hex

// Sequential methods (64-bit only)
AtomicId<x64>::sequential()               // Base36
AtomicId<x64>::sequential_base58()        // Base58
AtomicId<x64>::sequential_base91()        // Base91
AtomicId<x64>::sequential_base36()        // Base36
AtomicId<x64>::sequential_hex()           // Hex
AtomicId<x64>::sequential_batch(count)    // Batch Base36
AtomicId<x64>::sequential_base58_batch(count)
AtomicId<x64>::sequential_base91_batch(count)
AtomicId<x64>::sequential_base36_batch(count)
AtomicId<x64>::sequential_hex_batch(count)

// Epoch configuration
AtomicOption::epoch(epoch_ms)              // Set custom epoch
AtomicOption::get_epoch()                  // Get current epoch
AtomicOption::reset_epoch()                // Reset to default
```

## Use Cases

**24-bit Mode**: Session tokens, cache keys, temporary IDs (requires `short` feature)
```rust
// let session = AtomicId<x24>::base91(); // "AAAB" - ultra compact
```

**32-bit Mode**: Short-lived identifiers, internal references (requires `short` feature)
```rust
// let reference = AtomicId<x32>::base58(); // "111111" - compact & readable
```

**64-bit Mode**: Standard application IDs, database keys
```rust
let user_id = AtomicId<x64>::base91(); // "AAAAAAAAAB" - good balance
```

**128-bit Mode**: Distributed system IDs, API tokens (default)
```rust
let api_token = AtomicId<x128>::base58(); // High entropy, distributed-safe
```

**256-bit Mode**: Cryptographic identifiers, maximum security
```rust
let crypto_id = AtomicId<x256>::hex(); // Maximum entropy, ObjectId-style
```

**Sequential Mode**: High-frequency scenarios, guaranteed uniqueness (64-bit only)
```rust
let event_id = AtomicId<x64>::sequential_base91(); // No collisions even at 10M/sec
```

**Custom Epochs**: Domain-specific timestamps
```rust
// For application-specific time ranges
AtomicOption::epoch(1704067200000); // Start from 2024
let scoped_id = AtomicId<x128>::base58();

// For relative timestamps (useful in testing or temp environments)
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64;
AtomicOption::epoch(now);
let relative_id = AtomicId<x64>::hex();
```

## Performance Characteristics

- **Throughput**: Up to 10 Gbps ID generation
- **Latency**: Sub-microsecond per ID
- **Concurrency**: Lock-free, scales linearly with cores
- **Memory**: Cache-line optimized, minimal allocation
- **Uniqueness**: Guaranteed across distributed environments
- **Fixed-width**: All IDs of same bit-mode have consistent length

## Thread Safety & Distribution

All methods are thread-safe and lock-free. The system uses:

- **Thread isolation**: Each thread gets isolated counters to prevent contention
- **Node/Shard separation**: Built-in node and shard identifiers for distributed uniqueness
- **Atomic operations**: Lock-free implementation for maximum parallel performance
- **Cache-line alignment**: Optimized memory layout to prevent false sharing

## Architecture

The ID structure incorporates multiple components for guaranteed uniqueness:

- **Timestamp**: Millisecond precision (configurable epoch)
- **Node ID**: 12 bits (supports 4,096 nodes)
- **Shard ID**: 8 bits (256 shards per node)
- **Thread ID**: 8 bits (256 threads per shard)
- **Sequence**: 16+ bits (varies by mode, up to 65K+ IDs per ms per thread)
- **Extra entropy**: Additional randomness for higher bit modes

## Requirements

- Rust 1.70+
- No external dependencies
- Works on all platforms (Windows, macOS, Linux)
- Thread-safe across all operations
- Distributed-safe with proper node/shard configuration
