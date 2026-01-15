# Custom Channels

ALEC provides a channel abstraction for transport.

## Built-in Channels

```rust
use alec::Channel;

// Memory channel for testing
let (tx, rx) = Channel::memory(100);  // Buffer size 100

// Send
tx.send(&message)?;

// Receive
let received = rx.receive()?;
```

## Lossy Channel

Simulate unreliable networks:

```rust
let (tx, rx) = Channel::lossy(100, 0.1);  // 10% loss rate
```

## Channel Metrics

Track channel performance:

```rust
let metrics = channel.metrics();
println!("Sent: {}", metrics.messages_sent);
println!("Received: {}", metrics.messages_received);
println!("Dropped: {}", metrics.messages_dropped);
```

## Custom Implementation

Implement the `Channel` trait for custom transports:

```rust
use alec::channel::{Sender, Receiver};

struct MyTransport { /* ... */ }

impl Sender for MyTransport {
    fn send(&self, data: &[u8]) -> Result<(), Error> {
        // Your transport logic
    }
}
```
