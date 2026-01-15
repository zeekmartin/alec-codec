# Error Codes

ALEC error types and their meanings.

## DecodeError

Errors during message decoding:

| Error | Cause | Solution |
|-------|-------|----------|
| `BufferTooShort` | Message truncated | Check transmission |
| `InvalidHeader` | Corrupted header | Enable checksums |
| `UnknownEncodingType` | Protocol mismatch | Match ALEC versions |
| `ChecksumMismatch` | Data corrupted | Retry transmission |
| `InvalidVarint` | Malformed sequence | Check buffer handling |
| `ValueOutOfRange` | Impossible value | Verify encoder |

## ContextError

Errors in context operations:

| Error | Cause | Solution |
|-------|-------|----------|
| `DictionaryFull` | Too many patterns | Configure max_patterns |
| `PatternTooLarge` | Pattern exceeds limit | Reduce pattern size |
| `HashMismatch` | Sync verification failed | Full resync |
| `SyncFailed` | Sync operation error | Check sync messages |

## Error Handling

```rust
use alec::error::{DecodeError, AlecError};

match decoder.decode(&message, &context) {
    Ok(data) => process(data),
    Err(AlecError::Decode(DecodeError::ChecksumMismatch { .. })) => {
        // Request retransmission
    }
    Err(AlecError::Decode(DecodeError::BufferTooShort { .. })) => {
        // Wait for more data
    }
    Err(e) => {
        log::error!("Decode error: {}", e);
    }
}
```

## Recovery Strategies

| Error Type | Recovery |
|------------|----------|
| Checksum mismatch | Retry with backoff |
| Unknown encoding | Full resync |
| Context errors | Reset context |
| Transient | Circuit breaker |
