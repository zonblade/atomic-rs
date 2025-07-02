# Atomic-ID: High-Performance, Unique ID Generator for Rust

[![Crates.io](https://img.shields.io/crates/v/atomic-id.svg)](https://crates.io/crates/atomic-id)
[![Docs.rs](https://docs.rs/atomic-id/badge.svg)](https://docs.rs/atomic-id)

A high-performance, thread-safe, and distributed unique ID generator for Rust. Inspired by Snowflake, it's designed for applications requiring guaranteed uniqueness and high throughput.

## Features

- **Multiple Bit-Widths**: 24, 32, 64, 128, and 256-bit IDs.
- **Flexible Encodings**: Base36 (default), Base58, Base91, and Hexadecimal.
- **Lock-Free & Fast**: Uses atomics for high concurrency. Scales linearly with CPU cores.
- **Distributed-System Ready**: Incorporates node and shard IDs to prevent collisions in a distributed environment.
- **Customizable Epoch**: Allows setting a custom start time for timestamp-based IDs.
- **Batch Generation**: Optimized methods for creating IDs in bulk.

## Quick Start

1.  Add `atomic-id` to your `Cargo.toml`. For 24/32-bit IDs, enable the `short` feature.

    ```toml
    [dependencies]
    # For 64, 128, 256-bit IDs
    atomic-id = "0.1.0" 
    
    # Or, to include 24/32-bit support
    # atomic-id = { version = "0.1.0", features = ["short"] }
    ```

2.  Generate an ID. Here is a simple example using the `x64` (64-bit) mode:

    ```rust
    use atomic_id::{AtomicId, x64};

    // Generate a 64-bit ID using the default base36 encoding.
    let id = AtomicId::<x64>::new();
    println!("Generated 64-bit ID: {}", id);
    ```

    Other bit-widths (24, 32, 128, 256) and encodings (Base58, Base91, Hex) are also available. See the "Common Use Cases" and "API Overview" sections below for more details.

## Common Use Cases

Choose the right ID size and encoding for your needs.

| Bit Mode | Common Use Case                  | Example (Base58)                  |
|----------|----------------------------------|-----------------------------------|
| `x24`    | Temporary tokens, cache keys     | `AtomicId<x24>::base58()` (5 chars)  |
| `x32`    | Internal references, short links | `AtomicId<x32>::base58()` (6 chars)  |
| `x64`    | Database primary keys (e.g., `BIGINT`) | `AtomicId<x64>::base58()` (11 chars) |
| `x128`   | Distributed systems, API keys    | `AtomicId<x128>::base58()` (22 chars)|
| `x256`   | Cryptographic-level uniqueness   | `AtomicId<x256>::base58()` (44 chars)|

## Advanced Usage

### Sequential IDs

For high-frequency scenarios where you need strictly ordered, non-timestamp-based IDs, use the `sequential()` methods (64-bit only).

```rust
use atomic_id::{AtomicId, x64};

// These IDs are generated from a simple atomic counter.
let seq_id1 = AtomicId::<x64>::sequential();
let seq_id2 = AtomicId::<x64>::sequential();

// `seq_id2` is guaranteed to be greater than `seq_id1`.
```

### Custom Epoch

For timestamp-based IDs (64, 128, 256-bit), you can set a global epoch. This is useful for extending the ID lifespan or for creating application-specific time ranges.

```rust
use atomic_id::{AtomicOption, AtomicId, x64};

// Set the epoch to the start of 2024 (timestamp in milliseconds)
AtomicOption::epoch(1704067200000);

// All subsequent timestamp-based IDs will be relative to this new epoch.
let id = AtomicId::<x64>::new();

// You can also reset it to the default.
AtomicOption::reset_epoch();
```

## API Overview

The API is consistent across all supported bit-widths.

-   **Generation**:
    -   `AtomicId::<xBITS>::new()` (base36)
    -   `AtomicId::<xBITS>::base36()`
    -   `AtomicId::<xBITS>::base58()`
    -   `AtomicId::<xBITS>::base91()`
    -   `AtomicId::<xBITS>::hex()`
-   **Batch Generation**:
    -   `AtomicId::<xBITS>::batch(count)`
    -   `AtomicId::<xBITS>::base58_batch(count)`
    -   ...and so on for each encoding.
-   **Sequential (64-bit only)**:
    -   `AtomicId<x64>::sequential()`
    -   `AtomicId<x64>::sequential_base58()`
    -   ...and so on for each encoding.
-   **Configuration**:
    -   `AtomicOption::epoch(ms)`
    -   `AtomicOption::get_epoch()`
    -   `AtomicOption::reset_epoch()`

## ID Structure

The generator embeds several components into each ID to guarantee uniqueness, especially in distributed systems. A 64-bit ID, for example, is composed of:

-   **Timestamp** (20 bits): Milliseconds since the custom epoch.
-   **Node ID** (12 bits): Identifier for the machine or process.
-   **Shard ID** (8 bits): Identifier for a logical partition.
-   **Thread ID** (8 bits): Identifier for the generating thread.
-   **Sequence** (16 bits): A per-thread counter that resets every millisecond.

This structure prevents collisions even when multiple threads on multiple machines are generating IDs simultaneously.
