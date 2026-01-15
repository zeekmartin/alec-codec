# Context Management

The `Context` is the shared state between encoder and decoder.

## Creating a Context

```rust
use alec::Context;

// Default context
let context = Context::new();

// With custom configuration
use alec::context::ContextConfig;
let config = ContextConfig {
    max_patterns: 1000,
    max_memory: 32 * 1024,  // 32KB
    ..Default::default()
};
let context = Context::with_config(config);
```

## Updating Context

Always update context after encoding/decoding:

```rust
// After encoding
context.observe(&data);

// After decoding
context.observe(&decoded);
```

## Context Evolution

Context automatically evolves to improve compression:

```rust
use alec::context::EvolutionConfig;

let context = Context::with_evolution(EvolutionConfig {
    min_frequency: 2,        // Minimum usage to keep pattern
    max_age: 10000,          // Maximum age before pruning
    evolution_interval: 100, // Evolve every N observations
    enabled: true,
    ..Default::default()
});

// Or trigger manually
context.evolve();
```

## Predictions

Context provides predictions for better compression:

```rust
if let Some(prediction) = context.predict(source_id) {
    println!("Predicted: {}", prediction.value);
    println!("Confidence: {}", prediction.confidence);
}
```

## Memory Monitoring

Monitor context memory usage:

```rust
let memory = context.estimated_memory();
let patterns = context.pattern_count();

println!("Using {} bytes for {} patterns", memory, patterns);
```

## Exporting/Importing

For synchronization:

```rust
// Export on sender side
let data = context.export_full();

// Import on receiver side
context.import_full(&data)?;
```
