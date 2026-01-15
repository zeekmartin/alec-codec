# Prompt 05 — Synchronisation Automatique (v0.3.0)

## Contexte

Pour v0.3.0, les contextes émetteur/récepteur doivent se synchroniser automatiquement :
- Détecter la désynchronisation
- Échanger les diffs de dictionnaire
- Récupérer après divergence

## Objectif

Implémenter un protocole de synchronisation qui :
1. Détecte les divergences via hash
2. Envoie des messages SYNC incrémentaux
3. Gère les requêtes différées (REQ_DETAIL, REQ_RANGE)
4. Récupère automatiquement après perte de paquets

## Spécification

### Messages de synchronisation

```rust
pub enum SyncMessage {
    /// Announce current context state
    SyncAnnounce {
        version: u32,
        hash: u64,
        pattern_count: u16,
    },
    
    /// Request full or partial sync
    SyncRequest {
        from_version: u32,
        to_version: u32,
    },
    
    /// Send context diff
    SyncDiff {
        base_version: u32,
        new_version: u32,
        added_patterns: Vec<(u32, Pattern)>,
        removed_pattern_ids: Vec<u32>,
    },
    
    /// Request specific data that was compressed
    ReqDetail {
        sequence: u32,
    },
    
    /// Request range of historical data
    ReqRange {
        source_id: u32,
        from_timestamp: u64,
        to_timestamp: u64,
    },
    
    /// Response with detailed data
    DetailResponse {
        sequence: u32,
        full_data: RawData,
    },
}
```

### Protocole de synchronisation

```
Émetteur                          Récepteur
    |                                  |
    |------ DATA (ctx_version=5) ----->|
    |                                  | (détecte version mismatch)
    |<----- SYNC_REQUEST (from=3) -----|
    |                                  |
    |------ SYNC_DIFF (3→5) --------->|
    |                                  | (applique diff)
    |------ DATA (ctx_version=5) ----->|
    |                                  | (OK, peut décoder)
```

## Étapes

### 1. Créer `src/sync.rs`

```rust
//! Context synchronization protocol
//!
//! Handles automatic synchronization between encoder and decoder contexts.

use crate::context::Context;
use crate::protocol::{Pattern, RawData};
use crate::error::Result;

/// Synchronization message types
#[derive(Debug, Clone, PartialEq)]
pub enum SyncMessage {
    /// Periodic state announcement
    Announce(SyncAnnounce),
    /// Request synchronization
    Request(SyncRequest),
    /// Context diff
    Diff(SyncDiff),
    /// Request detailed data
    ReqDetail(u32),
    /// Request data range
    ReqRange(RangeRequest),
    /// Detail response
    DetailResponse(DetailResponse),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncAnnounce {
    pub version: u32,
    pub hash: u64,
    pub pattern_count: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncRequest {
    pub from_version: u32,
    pub to_version: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncDiff {
    pub base_version: u32,
    pub new_version: u32,
    pub added: Vec<(u32, Pattern)>,
    pub removed: Vec<u32>,
    pub hash: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RangeRequest {
    pub source_id: u32,
    pub from_timestamp: u64,
    pub to_timestamp: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetailResponse {
    pub sequence: u32,
    pub data: RawData,
}

/// Synchronization state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncState {
    /// Contexts are synchronized
    Synchronized,
    /// Waiting for sync response
    WaitingForSync { requested_at: u64 },
    /// Applying received diff
    Applying,
    /// Sync failed, need full resync
    Diverged,
}

/// Context synchronizer
#[derive(Debug)]
pub struct Synchronizer {
    /// Current sync state
    state: SyncState,
    /// Local context version
    local_version: u32,
    /// Remote context version (if known)
    remote_version: Option<u32>,
    /// Pending requests
    pending_requests: Vec<SyncRequest>,
    /// Configuration
    config: SyncConfig,
}

#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// How often to send announcements (in messages)
    pub announce_interval: u32,
    /// Max version gap before full resync
    pub max_version_gap: u32,
    /// Timeout for sync requests (in observations)
    pub sync_timeout: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            announce_interval: 100,
            max_version_gap: 10,
            sync_timeout: 1000,
        }
    }
}

impl Synchronizer {
    pub fn new() -> Self {
        Self {
            state: SyncState::Synchronized,
            local_version: 0,
            remote_version: None,
            pending_requests: Vec::new(),
            config: SyncConfig::default(),
        }
    }
    
    /// Check if sync is needed based on received message
    pub fn check_sync_needed(&mut self, remote_version: u32, remote_hash: u64, local: &Context) -> Option<SyncMessage> {
        // Version match and hash match = all good
        if remote_version == local.version() && remote_hash == local.hash() {
            self.state = SyncState::Synchronized;
            self.remote_version = Some(remote_version);
            return None;
        }
        
        // Version mismatch = need sync
        let gap = if remote_version > local.version() {
            remote_version - local.version()
        } else {
            local.version() - remote_version
        };
        
        if gap > self.config.max_version_gap {
            // Too far behind, need full resync
            self.state = SyncState::Diverged;
            return Some(SyncMessage::Request(SyncRequest {
                from_version: 0,
                to_version: Some(remote_version),
            }));
        }
        
        // Request incremental sync
        self.state = SyncState::WaitingForSync { requested_at: 0 };
        Some(SyncMessage::Request(SyncRequest {
            from_version: local.version(),
            to_version: Some(remote_version),
        }))
    }
    
    /// Generate a diff between two context versions
    pub fn generate_diff(old_ctx: &Context, new_ctx: &Context) -> SyncDiff {
        // This is simplified - real impl would track changes
        let added: Vec<_> = new_ctx.patterns_iter()
            .filter(|(id, _)| !old_ctx.has_pattern(**id))
            .map(|(id, p)| (*id, p.clone()))
            .collect();
            
        let removed: Vec<_> = old_ctx.pattern_ids()
            .filter(|id| !new_ctx.has_pattern(*id))
            .collect();
        
        SyncDiff {
            base_version: old_ctx.version(),
            new_version: new_ctx.version(),
            added,
            removed,
            hash: new_ctx.hash(),
        }
    }
    
    /// Apply a diff to a context
    pub fn apply_diff(context: &mut Context, diff: &SyncDiff) -> Result<()> {
        // Remove old patterns
        for id in &diff.removed {
            context.remove_pattern(*id);
        }
        
        // Add new patterns
        for (id, pattern) in &diff.added {
            context.set_pattern(*id, pattern.clone());
        }
        
        // Update version
        context.set_version(diff.new_version);
        
        // Verify hash
        if context.hash() != diff.hash {
            // Hash mismatch - sync failed
            return Err(crate::error::ContextError::HashMismatch {
                expected: diff.hash,
                actual: context.hash(),
            }.into());
        }
        
        Ok(())
    }
    
    /// Create an announcement message
    pub fn create_announce(context: &Context) -> SyncMessage {
        SyncMessage::Announce(SyncAnnounce {
            version: context.version(),
            hash: context.hash(),
            pattern_count: context.pattern_count() as u16,
        })
    }
}

impl Default for Synchronizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sync_announce() {
        let context = Context::new();
        let announce = Synchronizer::create_announce(&context);
        
        match announce {
            SyncMessage::Announce(a) => {
                assert_eq!(a.version, 0);
                assert_eq!(a.pattern_count, 0);
            }
            _ => panic!("Expected Announce"),
        }
    }
    
    #[test]
    fn test_sync_needed_detection() {
        let mut sync = Synchronizer::new();
        let local = Context::new();
        
        // Same version, same hash = no sync needed
        let result = sync.check_sync_needed(0, local.hash(), &local);
        assert!(result.is_none());
        
        // Different version = sync needed
        let result = sync.check_sync_needed(5, 12345, &local);
        assert!(result.is_some());
    }
    
    #[test]
    fn test_diff_generation() {
        let mut old_ctx = Context::new();
        let mut new_ctx = Context::new();
        
        // Add pattern to new context
        new_ctx.register_pattern(Pattern::new(42.0));
        
        let diff = Synchronizer::generate_diff(&old_ctx, &new_ctx);
        
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
    }
}
```

### 2. Ajouter méthodes au Context

```rust
impl Context {
    /// Remove a pattern by ID
    pub fn remove_pattern(&mut self, id: u32) {
        self.patterns.remove(&id);
    }
    
    /// Set a pattern at specific ID
    pub fn set_pattern(&mut self, id: u32, pattern: Pattern) {
        self.patterns.insert(id, PatternEntry::new(pattern, self.observation_count));
    }
    
    /// Check if pattern exists
    pub fn has_pattern(&self, id: u32) -> bool {
        self.patterns.contains_key(&id)
    }
    
    /// Iterate over patterns
    pub fn patterns_iter(&self) -> impl Iterator<Item = (&u32, &Pattern)> {
        self.patterns.iter().map(|(id, entry)| (id, &entry.pattern))
    }
    
    /// Get all pattern IDs
    pub fn pattern_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.patterns.keys().copied()
    }
    
    /// Set version directly (for sync)
    pub fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}
```

### 3. Intégrer avec Channel

Créer un canal avec sync automatique :

```rust
pub struct SyncChannel<C: Channel> {
    inner: C,
    synchronizer: Synchronizer,
    local_context: Context,
}

impl<C: Channel> SyncChannel<C> {
    pub fn send_with_sync(&mut self, message: EncodedMessage) -> Result<()> {
        // Periodically send announce
        if self.should_announce() {
            let announce = Synchronizer::create_announce(&self.local_context);
            self.inner.send(&announce.to_bytes())?;
        }
        
        self.inner.send(&message.to_bytes())
    }
    
    pub fn receive_with_sync(&mut self) -> Result<Option<EncodedMessage>> {
        let bytes = self.inner.receive()?;
        
        // Check if it's a sync message
        if let Some(sync_msg) = SyncMessage::from_bytes(&bytes) {
            self.handle_sync_message(sync_msg)?;
            return Ok(None);
        }
        
        // Regular data message
        Ok(EncodedMessage::from_bytes(&bytes))
    }
}
```

## Livrables

- [ ] `src/sync.rs` — Module de synchronisation
- [ ] Types `SyncMessage`, `SyncDiff`, etc.
- [ ] `Synchronizer` avec state machine
- [ ] Méthodes helper sur `Context`
- [ ] `SyncChannel` wrapper (optionnel)
- [ ] Tests (au moins 5)
- [ ] Sérialisation des messages sync

## Critères de succès

```bash
cargo test sync  # Tests de synchronisation
```

Scénarios à valider :
- Sync après démarrage
- Sync après perte de paquets
- Détection de divergence
- Recovery après désync

## Prochaine étape

→ `06_mode_flotte.md` (v0.4.0)
