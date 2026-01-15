# Protocol Specification

ALEC binary message format.

## Message Structure

```
┌─────────┬──────────┬─────────┬──────────┐
│ Header  │ Sequence │ Payload │ Checksum │
│ 1 byte  │ 1-5 bytes│ N bytes │ 4 bytes  │
└─────────┴──────────┴─────────┴──────────┘
```

## Header Byte

```
┌───┬───┬───┬───┬───┬───┬───┬───┐
│ 7 │ 6 │ 5 │ 4 │ 3 │ 2 │ 1 │ 0 │
├───┴───┴───┼───┴───┼───┴───┴───┤
│  Encoding │ Prio  │  MsgType  │
└───────────┴───────┴───────────┘
```

- **Encoding** (bits 5-7): Encoding type
- **Priority** (bits 3-4): P1-P5
- **MsgType** (bits 0-2): Message type

## Encoding Types

| Value | Name | Description |
|-------|------|-------------|
| 0 | Raw | Full 8-byte value |
| 1 | Delta | Difference from prediction |
| 2 | Repeated | Same as previous |
| 3 | Dictionary | Pattern reference |
| 4 | Multi | Multiple values |

## Sequence Number

Variable-length integer (varint):

```
Value < 128:      1 byte
Value < 16384:    2 bytes
Value < 2097152:  3 bytes
...
```

## Checksum

CRC32 (optional, 4 bytes at end of message).

## Example Messages

**Repeated (smallest):**
```
[Header: 1 byte][Seq: 1 byte] = 2 bytes
```

**Delta (typical):**
```
[Header: 1 byte][Seq: 1 byte][Delta: 1-4 bytes] = 3-6 bytes
```

**Raw (largest):**
```
[Header: 1 byte][Seq: 1 byte][Value: 8 bytes][Timestamp: 8 bytes] = 18 bytes
```
