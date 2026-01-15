# ALEC — Communication inter-composants

## Vue d'ensemble

Ce document définit les interfaces, protocoles et formats d'échange entre les composants internes d'ALEC, ainsi qu'avec les systèmes externes.

---

## Interfaces internes

### Vue des composants

```
┌─────────────────────────────────────────────────────────────────┐
│                          ÉMETTEUR                               │
│                                                                 │
│  ┌──────────┐    ┌─────────────┐    ┌──────────┐    ┌────────┐ │
│  │ ISource  │───▶│ IClassifier │───▶│ IEncoder │───▶│IChannel│ │
│  └──────────┘    └──────┬──────┘    └────┬─────┘    └────────┘ │
│                         │                │                      │
│                         └────────┬───────┘                      │
│                                  ▼                              │
│                          ┌─────────────┐                        │
│                          │  IContext   │                        │
│                          └─────────────┘                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## Définition des interfaces

### ISource — Source de données

```rust
/// Interface pour les sources de données
trait ISource {
    /// Type de donnée produite
    type Data;
    
    /// Récupère la prochaine donnée (bloquant ou async)
    fn next(&mut self) -> Option<Self::Data>;
    
    /// Récupère les métadonnées de la source
    fn metadata(&self) -> SourceMetadata;
}

struct SourceMetadata {
    source_id: u32,
    data_type: DataType,
    sampling_rate: Option<Duration>,
    location: Option<GeoLocation>,
}

enum DataType {
    Numeric { unit: String, precision: u8 },
    Binary { max_size: usize },
    Structured { schema_id: u32 },
}
```

### IClassifier — Classifieur de priorité

```rust
/// Interface pour la classification des données
trait IClassifier {
    /// Classifie une donnée et retourne sa priorité
    fn classify(&self, data: &RawData, context: &Context) -> Classification;
    
    /// Met à jour les seuils de classification
    fn update_thresholds(&mut self, thresholds: Thresholds);
}

struct Classification {
    priority: Priority,
    reason: ClassificationReason,
    delta: f64,           // Écart à la prédiction
    confidence: f32,      // 0.0 - 1.0
}

enum Priority {
    P1Critical,
    P2Important,
    P3Normal,
    P4Deferred,
    P5Disposable,
}

enum ClassificationReason {
    ThresholdExceeded { threshold: f64, actual: f64 },
    AnomalyDetected { anomaly_type: AnomalyType },
    ScheduledTransmission,
    UserRequested,
    ContextSync,
}
```

### IEncoder — Encodeur

```rust
/// Interface pour l'encodage des données
trait IEncoder {
    /// Encode une donnée classifiée
    fn encode(
        &self, 
        data: &RawData, 
        classification: &Classification,
        context: &Context
    ) -> EncodedMessage;
    
    /// Encode une mise à jour de contexte
    fn encode_context_update(&self, update: &ContextUpdate) -> EncodedMessage;
}

struct EncodedMessage {
    header: MessageHeader,
    payload: Vec<u8>,
    checksum: u32,
}

struct MessageHeader {
    version: u8,
    message_type: MessageType,
    priority: Priority,
    sequence: u32,
    timestamp: u64,
    context_version: u32,
}

enum MessageType {
    Data,
    ContextSync,
    Request,
    Response,
    Ack,
    Heartbeat,
}
```

### IDecoder — Décodeur

```rust
/// Interface pour le décodage des messages
trait IDecoder {
    /// Décode un message reçu
    fn decode(
        &self, 
        message: &EncodedMessage, 
        context: &Context
    ) -> Result<DecodedData, DecodeError>;
}

enum DecodeError {
    InvalidChecksum { expected: u32, actual: u32 },
    ContextMismatch { expected_version: u32, actual_version: u32 },
    MalformedMessage { offset: usize, reason: String },
    UnknownPattern { pattern_id: u32 },
}

struct DecodedData {
    source_id: u32,
    timestamp: u64,
    priority: Priority,
    value: DataValue,
    deferred_available: bool,  // Si des données P4/P5 sont disponibles
}
```

### IContext — Contexte partagé

```rust
/// Interface pour le contexte partagé
trait IContext {
    /// Prédit la prochaine valeur attendue
    fn predict(&self, source_id: u32) -> Option<Prediction>;
    
    /// Met à jour le contexte avec une nouvelle observation
    fn observe(&mut self, source_id: u32, value: &DataValue);
    
    /// Récupère le code court pour un pattern
    fn get_code(&self, pattern: &Pattern) -> Option<ShortCode>;
    
    /// Enregistre un nouveau pattern
    fn register_pattern(&mut self, pattern: Pattern) -> ShortCode;
    
    /// Calcule le diff depuis une version donnée
    fn diff_since(&self, version: u32) -> ContextDiff;
    
    /// Applique un diff reçu
    fn apply_diff(&mut self, diff: &ContextDiff) -> Result<(), SyncError>;
    
    /// Vérifie la cohérence avec un hash distant
    fn verify(&self, remote_hash: u64) -> bool;
}

struct Prediction {
    value: f64,
    confidence: f32,
    model_type: PredictionModel,
}

enum PredictionModel {
    LastValue,
    MovingAverage { window: usize },
    LinearRegression,
    Periodic { period: Duration },
    Custom { model_id: u32 },
}

struct ContextDiff {
    from_version: u32,
    to_version: u32,
    additions: Vec<(Pattern, ShortCode)>,
    removals: Vec<ShortCode>,
    model_updates: Vec<ModelUpdate>,
    hash: u64,
}
```

### IChannel — Canal de communication

```rust
/// Interface pour le canal de communication
trait IChannel {
    /// Envoie un message
    fn send(&mut self, message: EncodedMessage) -> Result<(), ChannelError>;
    
    /// Reçoit un message (bloquant avec timeout)
    fn receive(&mut self, timeout: Duration) -> Result<EncodedMessage, ChannelError>;
    
    /// Vérifie la disponibilité du canal
    fn is_available(&self) -> bool;
    
    /// Récupère les métriques du canal
    fn metrics(&self) -> ChannelMetrics;
}

struct ChannelMetrics {
    bytes_sent: u64,
    bytes_received: u64,
    messages_sent: u64,
    messages_received: u64,
    latency_avg_ms: f32,
    error_rate: f32,
    bandwidth_available: u32,  // bytes/sec estimé
}

enum ChannelError {
    Timeout,
    Disconnected,
    BufferFull,
    TransmissionError { retries: u8 },
}
```

### IRequestHandler — Gestionnaire de requêtes

```rust
/// Interface côté émetteur pour gérer les requêtes
trait IRequestHandler {
    /// Traite une requête entrante
    fn handle(&mut self, request: Request) -> Response;
    
    /// Vérifie si une requête est autorisée
    fn authorize(&self, request: &Request) -> bool;
}

enum Request {
    Detail { event_id: u64 },
    Range { from: u64, to: u64, source_id: Option<u32> },
    Resync { from_version: u32 },
    Status,
}

enum Response {
    Data { payload: Vec<u8> },
    ContextDiff { diff: ContextDiff },
    Status { status: EmitterStatus },
    Error { code: u16, message: String },
    RateLimited { retry_after: Duration },
}
```

---

## Format des messages binaires

### Structure générale

```
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Bytes 1-4   │ Bytes 5-8   │ Bytes 9-12  │ Variable  │
├───────────┼─────────────┼─────────────┼─────────────┼───────────┤
│ Header    │ Sequence    │ Timestamp   │ Ctx Version │ Payload   │
│ (flags)   │ (u32 BE)    │ (u32 BE)    │ (u32 BE)    │           │
└───────────┴─────────────┴─────────────┴─────────────┴───────────┘

Header byte:
┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐
│ Ver │ Ver │ Type│ Type│ Type│ Pri │ Pri │ Pri │
│ (1) │ (0) │ (2) │ (1) │ (0) │ (2) │ (1) │ (0) │
└─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘

Version: 2 bits (0-3)
Type: 3 bits (0-7)
Priority: 3 bits (0-7)
```

### Types de messages

| Code | Type | Description |
|------|------|-------------|
| 0 | DATA | Données encodées |
| 1 | SYNC | Synchronisation de contexte |
| 2 | REQ | Requête |
| 3 | RESP | Réponse |
| 4 | ACK | Accusé de réception |
| 5 | NACK | Accusé négatif |
| 6 | HEARTBEAT | Keep-alive |
| 7 | RESERVED | Usage futur |

### Payload DATA

```
┌──────────────┬───────────────┬─────────────────────────────────┐
│ Source ID    │ Encoding Type │ Value                           │
│ (varint)     │ (1 byte)      │ (variable)                      │
└──────────────┴───────────────┴─────────────────────────────────┘

Encoding Types:
0x00: Raw (valeur brute, 8 bytes float64)
0x01: Delta8 (delta sur 1 byte signé)
0x02: Delta16 (delta sur 2 bytes signé)
0x03: Pattern (référence au dictionnaire, varint)
0x04: Repeated (même valeur que précédent)
0x05: Interpolated (valeur prédite exacte, 0 bytes)
```

### Payload SYNC

```
┌──────────────┬──────────────┬──────────────┬───────────────────┐
│ From Version │ To Version   │ Hash         │ Operations        │
│ (u32 BE)     │ (u32 BE)     │ (u64 BE)     │ (variable)        │
└──────────────┴──────────────┴──────────────┴───────────────────┘

Operations:
┌──────┬──────────────┬─────────────────────────────────────────┐
│ Op   │ Code/Pattern │ Data                                    │
│ 1 b  │ varint       │ variable                                │
└──────┴──────────────┴─────────────────────────────────────────┘

Op types:
0x00: ADD (ajouter pattern)
0x01: REMOVE (supprimer code)
0x02: UPDATE_MODEL (mettre à jour modèle prédictif)
```

---

## Protocole de synchronisation

### Initialisation

```
Émetteur                                     Récepteur
    │                                             │
    │──────── SYNC (full context) ───────────────▶│
    │                                             │
    │◀─────────── ACK (hash OK) ──────────────────│
    │                                             │
    │           Session établie                   │
```

### Synchronisation incrémentale

```
Émetteur                                     Récepteur
    │                                             │
    │  (après N messages ou timer)                │
    │                                             │
    │──────── SYNC (diff v42→v43) ───────────────▶│
    │                                             │
    │◀─────────── ACK (hash match) ───────────────│
    │                                             │
    
    OU si hash mismatch:
    
    │◀─────────── NACK (expected hash) ───────────│
    │                                             │
    │──────── SYNC (full from v40) ──────────────▶│
    │                                             │
```

### Requête de détails

```
Émetteur                                     Récepteur
    │                                             │
    │────────── DATA (P2, event_id=123) ─────────▶│
    │                                             │
    │                        (décision de demander détails)
    │                                             │
    │◀───────── REQ (Detail, event_id=123) ───────│
    │                                             │
    │  (vérifie autorisation)                     │
    │  (récupère données P4)                      │
    │                                             │
    │────────── RESP (data P4) ──────────────────▶│
    │                                             │
```

---

## Événements internes

### Bus d'événements

Les composants communiquent via un bus d'événements interne :

```rust
enum InternalEvent {
    // Événements de données
    DataReceived { source_id: u32, data: RawData },
    DataClassified { source_id: u32, classification: Classification },
    DataEncoded { message: EncodedMessage },
    DataDecoded { data: DecodedData },
    
    // Événements de contexte
    ContextUpdated { version: u32, changes: usize },
    ContextSyncRequired { reason: SyncReason },
    ContextSyncCompleted { new_version: u32 },
    ContextMismatch { local_hash: u64, remote_hash: u64 },
    
    // Événements de canal
    ChannelConnected,
    ChannelDisconnected { reason: String },
    ChannelDegraded { bandwidth: u32 },
    
    // Événements de requête
    RequestReceived { request: Request },
    ResponseSent { request_id: u64 },
    
    // Événements système
    MemoryPressure { available_kb: u32 },
    BatteryLow { percent: u8 },
    Shutdown,
}
```

### Souscription aux événements

```rust
// Exemple d'utilisation
let mut bus = EventBus::new();

// Le classifier souscrit aux données brutes
bus.subscribe(|event| matches!(event, InternalEvent::DataReceived { .. }), 
    |event| classifier.on_data(event));

// Le logger souscrit à tout
bus.subscribe(|_| true, |event| logger.log(event));

// Le gestionnaire de sync souscrit aux changements de contexte
bus.subscribe(|event| matches!(event, InternalEvent::ContextUpdated { .. }),
    |event| sync_manager.check_sync_needed(event));
```

---

## APIs externes

### API REST (côté récepteur)

```yaml
openapi: 3.0.0
info:
  title: ALEC Receiver API
  version: 1.0.0

paths:
  /api/v1/status:
    get:
      summary: État du récepteur
      responses:
        200:
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Status'

  /api/v1/emitters:
    get:
      summary: Liste des émetteurs connectés
      
  /api/v1/emitters/{id}/context:
    get:
      summary: Contexte partagé avec un émetteur
      
  /api/v1/emitters/{id}/data:
    get:
      summary: Données récentes d'un émetteur
      parameters:
        - name: from
          in: query
          schema:
            type: string
            format: date-time
        - name: to
          in: query
          schema:
            type: string
            format: date-time
        - name: priority
          in: query
          schema:
            type: string
            enum: [P1, P2, P3, P4, P5]

  /api/v1/metrics:
    get:
      summary: Métriques de compression et performance
```

### Webhooks (notifications)

```json
{
  "event": "alert",
  "timestamp": "2025-01-15T10:30:00Z",
  "emitter_id": 42,
  "priority": "P1",
  "data": {
    "type": "temperature",
    "value": 85.5,
    "threshold": 80.0
  }
}
```

---

## Compatibilité et versioning

### Règles de compatibilité

1. **Backward compatible** : Un récepteur v1.1 comprend un émetteur v1.0
2. **Forward tolerant** : Ignorer les champs inconnus, pas d'erreur
3. **Version négociée** : Handshake initial pour établir la version commune

### Évolution des messages

```
Version 1: Header + Payload
Version 2: Header + Payload + Checksum (optionnel)
Version 3: Header + Payload + Checksum + Extensions (optionnel)
```

Les extensions sont des TLV (Type-Length-Value) ignorables :

```
┌──────────┬──────────┬─────────────────────────────────────────┐
│ Type     │ Length   │ Value                                   │
│ (1 byte) │ (varint) │ (variable)                              │
└──────────┴──────────┴─────────────────────────────────────────┘
```
