# ALEC JSON Schemas

Reference for all JSON output formats.

## MetricsSnapshot (v1)

Output from `alec-gateway` metrics module.

### Example

```json
{
  "version": 1,
  "timestamp_ms": 1706000000000,
  "window": {
    "kind": "time_ms",
    "value": 60000,
    "aligned_samples": 50,
    "channels_included": 4
  },
  "signal": {
    "valid": true,
    "log_base": "log2",
    "h_per_channel": [
      { "channel_id": "temp", "h": 3.21 },
      { "channel_id": "humid", "h": 2.87 }
    ],
    "sum_h": 6.08,
    "h_joint": 5.12,
    "total_corr": 0.96
  },
  "payload": {
    "frame_size_bytes": 48,
    "h_bytes": 6.21
  },
  "resilience": {
    "enabled": true,
    "r": 0.158,
    "zone": "critical",
    "criticality": {
      "enabled": true,
      "ranking": [
        { "channel_id": "temp", "delta_r": 0.12 },
        { "channel_id": "humid", "delta_r": 0.08 }
      ],
      "note": "delta_r = R_all - R_without_channel (leave-one-out)"
    }
  },
  "flags": ["SIGNAL_COMPUTED"]
}
```

### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "MetricsSnapshot",
  "type": "object",
  "required": ["version", "timestamp_ms", "window", "signal", "payload", "flags"],
  "properties": {
    "version": { "type": "integer", "const": 1 },
    "timestamp_ms": { "type": "integer", "description": "UTC epoch milliseconds" },
    "window": {
      "type": "object",
      "required": ["kind", "value", "aligned_samples", "channels_included"],
      "properties": {
        "kind": { "type": "string", "enum": ["time_ms", "last_n"] },
        "value": { "type": "integer" },
        "aligned_samples": { "type": "integer" },
        "channels_included": { "type": "integer" }
      }
    },
    "signal": {
      "type": "object",
      "required": ["valid", "log_base", "sum_h", "h_joint", "total_corr"],
      "properties": {
        "valid": { "type": "boolean" },
        "invalid_reason": { "type": "string" },
        "log_base": { "type": "string", "enum": ["log2", "ln"] },
        "h_per_channel": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["channel_id", "h"],
            "properties": {
              "channel_id": { "type": "string" },
              "h": { "type": "number" }
            }
          }
        },
        "sum_h": { "type": "number" },
        "h_joint": { "type": "number" },
        "total_corr": { "type": "number" }
      }
    },
    "payload": {
      "type": "object",
      "required": ["frame_size_bytes", "h_bytes"],
      "properties": {
        "frame_size_bytes": { "type": "integer" },
        "h_bytes": { "type": "number" },
        "histogram": {
          "type": "array",
          "items": { "type": "integer" },
          "minItems": 256,
          "maxItems": 256
        },
        "per_channel": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["channel_id", "size_bytes", "h_bytes"],
            "properties": {
              "channel_id": { "type": "string" },
              "size_bytes": { "type": "integer" },
              "h_bytes": { "type": "number" }
            }
          }
        }
      }
    },
    "resilience": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean" },
        "r": { "type": ["number", "null"] },
        "zone": { "type": "string", "enum": ["healthy", "attention", "critical"] },
        "criticality": {
          "type": "object",
          "properties": {
            "enabled": { "type": "boolean" },
            "ranking": {
              "type": "array",
              "items": {
                "type": "object",
                "required": ["channel_id", "delta_r"],
                "properties": {
                  "channel_id": { "type": "string" },
                  "delta_r": { "type": "number" }
                }
              }
            },
            "note": { "type": "string" }
          }
        }
      }
    },
    "flags": {
      "type": "array",
      "items": { "type": "string" }
    }
  }
}
```

## ComplexitySnapshot (v0.1.0)

Output from `alec-complexity` engine.

### Example

```json
{
  "version": "0.1.0",
  "timestamp_ms": 1706000000000,
  "baseline": {
    "state": "locked",
    "sample_count": 25,
    "progress": 1.0,
    "stats": {
      "tc_mean": 2.1,
      "tc_std": 0.3,
      "h_joint_mean": 5.2,
      "h_joint_std": 0.5,
      "h_bytes_mean": 6.2,
      "h_bytes_std": 0.4,
      "r_mean": 0.45,
      "r_std": 0.08
    }
  },
  "deltas": {
    "tc": 0.7,
    "h_joint": 0.8,
    "h_bytes": 0.9,
    "r": -0.1
  },
  "z_scores": {
    "tc": 2.33,
    "h_joint": 1.60,
    "h_bytes": 2.25,
    "r": -1.25
  },
  "s_lite": {
    "edges": [
      { "channel_a": "temp", "channel_b": "humid", "weight": 0.72 }
    ],
    "channel_count": 4,
    "timestamp_ms": 1706000000000
  },
  "events": [
    {
      "event_type": "ComplexitySurge",
      "severity": "Warning",
      "timestamp_ms": 1706000000000,
      "details": {
        "field": "tc",
        "z_score": 2.33,
        "threshold": 2.0
      }
    }
  ],
  "flags": ["ANOMALY_ENABLED"]
}
```

### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "ComplexitySnapshot",
  "type": "object",
  "required": ["version", "timestamp_ms", "baseline"],
  "properties": {
    "version": { "type": "string" },
    "timestamp_ms": { "type": "integer" },
    "baseline": {
      "type": "object",
      "required": ["state", "sample_count", "progress"],
      "properties": {
        "state": { "type": "string", "enum": ["building", "locked"] },
        "sample_count": { "type": "integer" },
        "progress": { "type": "number", "minimum": 0, "maximum": 1 },
        "stats": { "$ref": "#/definitions/BaselineStats" }
      }
    },
    "deltas": { "$ref": "#/definitions/Deltas" },
    "z_scores": { "$ref": "#/definitions/ZScores" },
    "s_lite": { "$ref": "#/definitions/SLite" },
    "events": {
      "type": "array",
      "items": { "$ref": "#/definitions/ComplexityEvent" }
    },
    "flags": {
      "type": "array",
      "items": { "type": "string" }
    }
  },
  "definitions": {
    "BaselineStats": {
      "type": "object",
      "required": ["h_bytes_mean", "h_bytes_std"],
      "properties": {
        "tc_mean": { "type": "number" },
        "tc_std": { "type": "number" },
        "h_joint_mean": { "type": "number" },
        "h_joint_std": { "type": "number" },
        "h_bytes_mean": { "type": "number" },
        "h_bytes_std": { "type": "number" },
        "r_mean": { "type": "number" },
        "r_std": { "type": "number" }
      }
    },
    "Deltas": {
      "type": "object",
      "required": ["h_bytes"],
      "properties": {
        "tc": { "type": "number" },
        "h_joint": { "type": "number" },
        "h_bytes": { "type": "number" },
        "r": { "type": "number" }
      }
    },
    "ZScores": {
      "type": "object",
      "required": ["h_bytes"],
      "properties": {
        "tc": { "type": "number" },
        "h_joint": { "type": "number" },
        "h_bytes": { "type": "number" },
        "r": { "type": "number" }
      }
    },
    "SLite": {
      "type": "object",
      "required": ["edges", "channel_count", "timestamp_ms"],
      "properties": {
        "edges": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["channel_a", "channel_b", "weight"],
            "properties": {
              "channel_a": { "type": "string" },
              "channel_b": { "type": "string" },
              "weight": { "type": "number" }
            }
          }
        },
        "channel_count": { "type": "integer" },
        "timestamp_ms": { "type": "integer" }
      }
    },
    "ComplexityEvent": {
      "type": "object",
      "required": ["event_type", "severity", "timestamp_ms"],
      "properties": {
        "event_type": {
          "type": "string",
          "enum": [
            "BaselineBuilding",
            "BaselineLocked",
            "PayloadEntropySpike",
            "StructureBreak",
            "RedundancyDrop",
            "ComplexitySurge",
            "SensorCriticalityShift"
          ]
        },
        "severity": {
          "type": "string",
          "enum": ["Info", "Warning", "Critical"]
        },
        "timestamp_ms": { "type": "integer" },
        "details": { "type": "object" }
      }
    }
  }
}
```

## GenericInput (Complexity Input)

Input format for standalone `alec-complexity` usage.

### Example

```json
{
  "timestamp_ms": 1706000000000,
  "h_bytes": 6.5,
  "tc": 2.3,
  "h_joint": 8.1,
  "r": 0.45,
  "channels": [
    { "id": "temp", "h": 3.2 },
    { "id": "humid", "h": 2.8 }
  ]
}
```

### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "GenericInput",
  "type": "object",
  "required": ["timestamp_ms", "h_bytes"],
  "properties": {
    "timestamp_ms": { "type": "integer", "description": "UTC epoch milliseconds" },
    "h_bytes": { "type": "number", "description": "Payload entropy (bits)" },
    "tc": { "type": "number", "description": "Total Correlation (bits)" },
    "h_joint": { "type": "number", "description": "Joint entropy (bits)" },
    "r": { "type": "number", "description": "Resilience index (0-1)" },
    "channels": {
      "type": "array",
      "description": "Per-channel entropy for S-lite",
      "items": {
        "type": "object",
        "required": ["id", "h"],
        "properties": {
          "id": { "type": "string", "description": "Channel identifier" },
          "h": { "type": "number", "description": "Channel entropy (bits)" }
        }
      }
    }
  }
}
```

### Minimal Example

```json
{
  "timestamp_ms": 1706000000000,
  "h_bytes": 6.5
}
```

## Field Descriptions

### Common Fields

| Field | Type | Description |
|-------|------|-------------|
| `timestamp_ms` | integer | UTC epoch milliseconds |
| `version` | string/integer | Schema version |

### Entropy Fields

| Field | Description | Unit |
|-------|-------------|------|
| `h` / `h_bytes` | Shannon entropy | bits |
| `h_joint` | Joint entropy of all channels | bits |
| `total_corr` / `tc` | Total Correlation (redundancy) | bits |
| `sum_h` | Sum of individual entropies | bits |

### Resilience Fields

| Field | Description | Range |
|-------|-------------|-------|
| `r` | Resilience index | 0.0 - 1.0 |
| `zone` | Resilience zone | healthy/attention/critical |
| `delta_r` | Criticality (leave-one-out) | typically 0.0 - 0.3 |

### Z-Score Fields

| Field | Description | Interpretation |
|-------|-------------|----------------|
| `z_scores.tc` | Z-score for TC | \|z\| < 2: normal |
| `z_scores.h_bytes` | Z-score for H_bytes | \|z\| ≥ 3: critical |
