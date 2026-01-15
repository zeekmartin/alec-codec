//! Context synchronization protocol
//!
//! Handles automatic synchronization between encoder and decoder contexts.
//! This module provides:
//! - Sync message types for protocol communication
//! - State machine for tracking synchronization status
//! - Diff generation and application for incremental updates

use crate::context::{Context, Pattern};
use crate::error::{ContextError, Result};
use crate::protocol::RawData;

/// Synchronization message types
#[derive(Debug, Clone, PartialEq)]
pub enum SyncMessage {
    /// Periodic state announcement
    Announce(SyncAnnounce),
    /// Request synchronization
    Request(SyncRequest),
    /// Context diff
    Diff(SyncDiff),
    /// Request detailed data for a sequence
    ReqDetail(u32),
    /// Request data range
    ReqRange(RangeRequest),
    /// Detail response with full data
    DetailResponse(DetailResponse),
}

/// Announcement of current context state
#[derive(Debug, Clone, PartialEq)]
pub struct SyncAnnounce {
    /// Current context version
    pub version: u32,
    /// Hash of the context for verification
    pub hash: u64,
    /// Number of patterns in dictionary
    pub pattern_count: u16,
}

/// Request for synchronization
#[derive(Debug, Clone, PartialEq)]
pub struct SyncRequest {
    /// Starting version for incremental sync
    pub from_version: u32,
    /// Target version (None = latest)
    pub to_version: Option<u32>,
}

/// Context diff for incremental synchronization
#[derive(Debug, Clone, PartialEq)]
pub struct SyncDiff {
    /// Base version this diff applies to
    pub base_version: u32,
    /// New version after applying diff
    pub new_version: u32,
    /// Patterns added (id, pattern)
    pub added: Vec<(u32, Pattern)>,
    /// Pattern IDs removed
    pub removed: Vec<u32>,
    /// Hash of the resulting context
    pub hash: u64,
}

/// Request for a range of historical data
#[derive(Debug, Clone, PartialEq)]
pub struct RangeRequest {
    /// Source ID to request data from
    pub source_id: u32,
    /// Start timestamp
    pub from_timestamp: u64,
    /// End timestamp
    pub to_timestamp: u64,
}

/// Response with detailed data
#[derive(Debug, Clone, PartialEq)]
pub struct DetailResponse {
    /// Sequence number of the original message
    pub sequence: u32,
    /// Full uncompressed data
    pub data: RawData,
}

/// Synchronization state machine states
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SyncState {
    /// Contexts are synchronized
    #[default]
    Synchronized,
    /// Waiting for sync response
    WaitingForSync {
        /// When the request was made (observation count)
        requested_at: u64,
    },
    /// Currently applying received diff
    Applying,
    /// Sync failed, need full resync
    Diverged,
}

/// Configuration for synchronization behavior
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// How often to send announcements (in messages)
    pub announce_interval: u32,
    /// Max version gap before full resync
    pub max_version_gap: u32,
    /// Timeout for sync requests (in observations)
    pub sync_timeout: u64,
    /// Whether to automatically request sync on mismatch
    pub auto_sync: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            announce_interval: 100,
            max_version_gap: 10,
            sync_timeout: 1000,
            auto_sync: true,
        }
    }
}

/// Context synchronizer - manages sync state and operations
#[derive(Debug)]
pub struct Synchronizer {
    /// Current sync state
    state: SyncState,
    /// Local context version (cached)
    local_version: u32,
    /// Remote context version (if known)
    remote_version: Option<u32>,
    /// Number of messages since last announce
    messages_since_announce: u32,
    /// Pending sync requests
    pending_requests: Vec<SyncRequest>,
    /// Configuration
    config: SyncConfig,
}

impl Synchronizer {
    /// Create a new synchronizer with default configuration
    pub fn new() -> Self {
        Self {
            state: SyncState::Synchronized,
            local_version: 0,
            remote_version: None,
            messages_since_announce: 0,
            pending_requests: Vec::new(),
            config: SyncConfig::default(),
        }
    }

    /// Create a synchronizer with custom configuration
    pub fn with_config(config: SyncConfig) -> Self {
        Self {
            state: SyncState::Synchronized,
            local_version: 0,
            remote_version: None,
            messages_since_announce: 0,
            pending_requests: Vec::new(),
            config,
        }
    }

    /// Get current sync state
    pub fn state(&self) -> &SyncState {
        &self.state
    }

    /// Check if contexts are synchronized
    pub fn is_synchronized(&self) -> bool {
        matches!(self.state, SyncState::Synchronized)
    }

    /// Update local version cache
    pub fn update_local_version(&mut self, version: u32) {
        self.local_version = version;
    }

    /// Check if an announcement should be sent
    pub fn should_announce(&mut self) -> bool {
        self.messages_since_announce += 1;
        if self.messages_since_announce >= self.config.announce_interval {
            self.messages_since_announce = 0;
            true
        } else {
            false
        }
    }

    /// Check if sync is needed based on received announcement
    pub fn check_sync_needed(
        &mut self,
        remote_version: u32,
        remote_hash: u64,
        local: &Context,
    ) -> Option<SyncMessage> {
        // Version and hash match = all good
        if remote_version == local.version() && remote_hash == local.hash() {
            self.state = SyncState::Synchronized;
            self.remote_version = Some(remote_version);
            return None;
        }

        // Calculate version gap
        let gap = remote_version.abs_diff(local.version());

        if gap > self.config.max_version_gap {
            // Too far behind, need full resync
            self.state = SyncState::Diverged;
            return Some(SyncMessage::Request(SyncRequest {
                from_version: 0,
                to_version: Some(remote_version),
            }));
        }

        // Hash mismatch with same version = diverged
        if remote_version == local.version() && remote_hash != local.hash() {
            self.state = SyncState::Diverged;
            return Some(SyncMessage::Request(SyncRequest {
                from_version: 0,
                to_version: Some(remote_version),
            }));
        }

        // Request incremental sync
        self.state = SyncState::WaitingForSync { requested_at: 0 };
        self.remote_version = Some(remote_version);

        if self.config.auto_sync {
            Some(SyncMessage::Request(SyncRequest {
                from_version: local.version(),
                to_version: Some(remote_version),
            }))
        } else {
            None
        }
    }

    /// Handle a received sync request
    pub fn handle_request(
        &mut self,
        request: &SyncRequest,
        local: &Context,
    ) -> Option<SyncMessage> {
        // Generate diff from requested version to current
        let diff = Self::generate_diff_from_version(request.from_version, local);
        Some(SyncMessage::Diff(diff))
    }

    /// Generate a diff between two context versions
    /// Note: This is a simplified implementation that generates full diff
    pub fn generate_diff(old_ctx: &Context, new_ctx: &Context) -> SyncDiff {
        let added: Vec<_> = new_ctx
            .patterns_iter()
            .filter(|(id, _)| !old_ctx.has_pattern(**id))
            .map(|(id, p)| (*id, p.clone()))
            .collect();

        let removed: Vec<_> = old_ctx
            .pattern_ids()
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

    /// Generate diff from a specific version
    /// Simplified: exports all patterns since we don't track history
    fn generate_diff_from_version(from_version: u32, ctx: &Context) -> SyncDiff {
        let added: Vec<_> = if from_version == 0 {
            // Full sync requested
            ctx.patterns_iter()
                .map(|(id, p)| (*id, p.clone()))
                .collect()
        } else {
            // Incremental - return all patterns (simplified)
            ctx.patterns_iter()
                .map(|(id, p)| (*id, p.clone()))
                .collect()
        };

        SyncDiff {
            base_version: from_version,
            new_version: ctx.version(),
            added,
            removed: Vec::new(),
            hash: ctx.hash(),
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
        let actual_hash = context.hash();
        if actual_hash != diff.hash {
            return Err(ContextError::HashMismatch {
                expected: diff.hash,
                actual: actual_hash,
            }
            .into());
        }

        Ok(())
    }

    /// Handle a received diff
    pub fn handle_diff(&mut self, diff: &SyncDiff, context: &mut Context) -> Result<()> {
        self.state = SyncState::Applying;

        match Self::apply_diff(context, diff) {
            Ok(()) => {
                self.state = SyncState::Synchronized;
                self.local_version = context.version();
                Ok(())
            }
            Err(e) => {
                self.state = SyncState::Diverged;
                Err(e)
            }
        }
    }

    /// Create an announcement message for a context
    pub fn create_announce(context: &Context) -> SyncMessage {
        SyncMessage::Announce(SyncAnnounce {
            version: context.version(),
            hash: context.hash(),
            pattern_count: context.pattern_count() as u16,
        })
    }

    /// Create a sync request message
    pub fn create_request(from_version: u32, to_version: Option<u32>) -> SyncMessage {
        SyncMessage::Request(SyncRequest {
            from_version,
            to_version,
        })
    }

    /// Create a detail request message
    pub fn create_detail_request(sequence: u32) -> SyncMessage {
        SyncMessage::ReqDetail(sequence)
    }

    /// Create a range request message
    pub fn create_range_request(
        source_id: u32,
        from_timestamp: u64,
        to_timestamp: u64,
    ) -> SyncMessage {
        SyncMessage::ReqRange(RangeRequest {
            source_id,
            from_timestamp,
            to_timestamp,
        })
    }

    /// Create a detail response message
    pub fn create_detail_response(sequence: u32, data: RawData) -> SyncMessage {
        SyncMessage::DetailResponse(DetailResponse { sequence, data })
    }

    /// Check if sync has timed out
    pub fn check_timeout(&mut self, current_time: u64) -> bool {
        if let SyncState::WaitingForSync { requested_at } = self.state {
            if current_time - requested_at > self.config.sync_timeout {
                self.state = SyncState::Diverged;
                return true;
            }
        }
        false
    }

    /// Reset synchronizer state
    pub fn reset(&mut self) {
        self.state = SyncState::Synchronized;
        self.local_version = 0;
        self.remote_version = None;
        self.messages_since_announce = 0;
        self.pending_requests.clear();
    }
}

impl Default for Synchronizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Message type identifier for serialization
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMessageType {
    Announce = 0x10,
    Request = 0x11,
    Diff = 0x12,
    ReqDetail = 0x13,
    ReqRange = 0x14,
    DetailResponse = 0x15,
}

impl SyncMessage {
    /// Get the message type identifier
    pub fn message_type(&self) -> SyncMessageType {
        match self {
            SyncMessage::Announce(_) => SyncMessageType::Announce,
            SyncMessage::Request(_) => SyncMessageType::Request,
            SyncMessage::Diff(_) => SyncMessageType::Diff,
            SyncMessage::ReqDetail(_) => SyncMessageType::ReqDetail,
            SyncMessage::ReqRange(_) => SyncMessageType::ReqRange,
            SyncMessage::DetailResponse(_) => SyncMessageType::DetailResponse,
        }
    }

    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        match self {
            SyncMessage::Announce(a) => {
                bytes.push(SyncMessageType::Announce as u8);
                bytes.extend_from_slice(&a.version.to_be_bytes());
                bytes.extend_from_slice(&a.hash.to_be_bytes());
                bytes.extend_from_slice(&a.pattern_count.to_be_bytes());
            }
            SyncMessage::Request(r) => {
                bytes.push(SyncMessageType::Request as u8);
                bytes.extend_from_slice(&r.from_version.to_be_bytes());
                bytes.push(r.to_version.is_some() as u8);
                if let Some(to) = r.to_version {
                    bytes.extend_from_slice(&to.to_be_bytes());
                }
            }
            SyncMessage::Diff(d) => {
                bytes.push(SyncMessageType::Diff as u8);
                bytes.extend_from_slice(&d.base_version.to_be_bytes());
                bytes.extend_from_slice(&d.new_version.to_be_bytes());
                bytes.extend_from_slice(&d.hash.to_be_bytes());
                bytes.extend_from_slice(&(d.added.len() as u16).to_be_bytes());
                for (id, pattern) in &d.added {
                    bytes.extend_from_slice(&id.to_be_bytes());
                    bytes.push(pattern.data.len() as u8);
                    bytes.extend_from_slice(&pattern.data);
                }
                bytes.extend_from_slice(&(d.removed.len() as u16).to_be_bytes());
                for id in &d.removed {
                    bytes.extend_from_slice(&id.to_be_bytes());
                }
            }
            SyncMessage::ReqDetail(seq) => {
                bytes.push(SyncMessageType::ReqDetail as u8);
                bytes.extend_from_slice(&seq.to_be_bytes());
            }
            SyncMessage::ReqRange(r) => {
                bytes.push(SyncMessageType::ReqRange as u8);
                bytes.extend_from_slice(&r.source_id.to_be_bytes());
                bytes.extend_from_slice(&r.from_timestamp.to_be_bytes());
                bytes.extend_from_slice(&r.to_timestamp.to_be_bytes());
            }
            SyncMessage::DetailResponse(d) => {
                bytes.push(SyncMessageType::DetailResponse as u8);
                bytes.extend_from_slice(&d.sequence.to_be_bytes());
                bytes.extend_from_slice(&d.data.value.to_be_bytes());
                bytes.extend_from_slice(&d.data.timestamp.to_be_bytes());
                bytes.extend_from_slice(&d.data.source_id.to_be_bytes());
            }
        }

        bytes
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        let msg_type = bytes[0];
        let data = &bytes[1..];

        match msg_type {
            0x10 if data.len() >= 14 => {
                let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let hash = u64::from_be_bytes([
                    data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
                ]);
                let pattern_count = u16::from_be_bytes([data[12], data[13]]);
                Some(SyncMessage::Announce(SyncAnnounce {
                    version,
                    hash,
                    pattern_count,
                }))
            }
            0x11 if data.len() >= 5 => {
                let from_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let has_to = data[4] != 0;
                let to_version = if has_to && data.len() >= 9 {
                    Some(u32::from_be_bytes([data[5], data[6], data[7], data[8]]))
                } else {
                    None
                };
                Some(SyncMessage::Request(SyncRequest {
                    from_version,
                    to_version,
                }))
            }
            0x13 if data.len() >= 4 => {
                let seq = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                Some(SyncMessage::ReqDetail(seq))
            }
            0x14 if data.len() >= 20 => {
                let source_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let from_timestamp = u64::from_be_bytes([
                    data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
                ]);
                let to_timestamp = u64::from_be_bytes([
                    data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
                ]);
                Some(SyncMessage::ReqRange(RangeRequest {
                    source_id,
                    from_timestamp,
                    to_timestamp,
                }))
            }
            0x15 if data.len() >= 20 => {
                let sequence = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let value = f64::from_be_bytes([
                    data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
                ]);
                let timestamp = u64::from_be_bytes([
                    data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
                ]);
                let source_id = if data.len() >= 24 {
                    u32::from_be_bytes([data[20], data[21], data[22], data[23]])
                } else {
                    0
                };
                Some(SyncMessage::DetailResponse(DetailResponse {
                    sequence,
                    data: RawData::with_source(source_id, value, timestamp),
                }))
            }
            // Diff parsing is more complex, simplified here
            0x12 if data.len() >= 20 => {
                let base_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let new_version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let hash = u64::from_be_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                let added_count = u16::from_be_bytes([data[16], data[17]]) as usize;

                let mut offset = 18;
                let mut added = Vec::with_capacity(added_count);

                for _ in 0..added_count {
                    if offset + 5 > data.len() {
                        return None;
                    }
                    let id = u32::from_be_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    let len = data[offset + 4] as usize;
                    offset += 5;

                    if offset + len > data.len() {
                        return None;
                    }
                    let pattern_data = data[offset..offset + len].to_vec();
                    offset += len;

                    added.push((id, Pattern::new(pattern_data)));
                }

                if offset + 2 > data.len() {
                    return None;
                }
                let removed_count = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2;

                let mut removed = Vec::with_capacity(removed_count);
                for _ in 0..removed_count {
                    if offset + 4 > data.len() {
                        return None;
                    }
                    let id = u32::from_be_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    offset += 4;
                    removed.push(id);
                }

                Some(SyncMessage::Diff(SyncDiff {
                    base_version,
                    new_version,
                    added,
                    removed,
                    hash,
                }))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_announce_creation() {
        let context = Context::new();
        let announce = Synchronizer::create_announce(&context);

        match announce {
            SyncMessage::Announce(a) => {
                assert_eq!(a.version, 0);
                assert_eq!(a.pattern_count, 0);
            }
            _ => panic!("Expected Announce message"),
        }
    }

    #[test]
    fn test_sync_needed_detection_same_state() {
        let mut sync = Synchronizer::new();
        let local = Context::new();

        // Same version, same hash = no sync needed
        let result = sync.check_sync_needed(0, local.hash(), &local);
        assert!(result.is_none());
        assert!(sync.is_synchronized());
    }

    #[test]
    fn test_sync_needed_detection_version_mismatch() {
        let mut sync = Synchronizer::new();
        let local = Context::new();

        // Different version = sync needed
        let result = sync.check_sync_needed(5, 12345, &local);
        assert!(result.is_some());

        match result.unwrap() {
            SyncMessage::Request(req) => {
                assert_eq!(req.from_version, 0);
                assert_eq!(req.to_version, Some(5));
            }
            _ => panic!("Expected Request message"),
        }
    }

    #[test]
    fn test_sync_needed_large_gap() {
        let mut sync = Synchronizer::with_config(SyncConfig {
            max_version_gap: 5,
            ..Default::default()
        });
        let local = Context::new();

        // Gap > max_version_gap = diverged
        let result = sync.check_sync_needed(100, 12345, &local);
        assert!(result.is_some());
        assert_eq!(sync.state, SyncState::Diverged);
    }

    #[test]
    fn test_diff_generation() {
        let old_ctx = Context::new();
        let mut new_ctx = Context::new();

        // Add pattern to new context
        new_ctx
            .register_pattern(Pattern::new(vec![1, 2, 3]))
            .unwrap();

        let diff = Synchronizer::generate_diff(&old_ctx, &new_ctx);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.base_version, 0);
        assert_eq!(diff.new_version, new_ctx.version());
    }

    #[test]
    fn test_diff_application() {
        let mut ctx1 = Context::new();
        let mut ctx2 = Context::new();

        // Add pattern to ctx1
        ctx1.register_pattern(Pattern::new(vec![1, 2, 3])).unwrap();

        // Generate and apply diff
        let diff = Synchronizer::generate_diff(&Context::new(), &ctx1);
        Synchronizer::apply_diff(&mut ctx2, &diff).unwrap();

        // Contexts should now match
        assert_eq!(ctx1.hash(), ctx2.hash());
        assert_eq!(ctx1.pattern_count(), ctx2.pattern_count());
    }

    #[test]
    fn test_sync_state_transitions() {
        let mut sync = Synchronizer::new();
        let mut local = Context::new();

        // Initially synchronized
        assert!(sync.is_synchronized());

        // Detect version mismatch
        sync.check_sync_needed(5, 12345, &local);
        assert!(!sync.is_synchronized());

        // Apply diff (simulate receiving response)
        let diff = SyncDiff {
            base_version: 0,
            new_version: 5,
            added: vec![],
            removed: vec![],
            hash: local.hash(), // Simplified - in real use would be different
        };

        // Need to set version first since diff expects version 5
        local.set_version(5);
        let new_diff = SyncDiff {
            hash: local.hash(),
            ..diff
        };

        sync.handle_diff(&new_diff, &mut local).unwrap();
        assert!(sync.is_synchronized());
    }

    #[test]
    fn test_announce_serialization() {
        let announce = SyncMessage::Announce(SyncAnnounce {
            version: 42,
            hash: 0x123456789ABCDEF0,
            pattern_count: 100,
        });

        let bytes = announce.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();

        assert_eq!(announce, parsed);
    }

    #[test]
    fn test_request_serialization() {
        let request = SyncMessage::Request(SyncRequest {
            from_version: 10,
            to_version: Some(20),
        });

        let bytes = request.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();

        assert_eq!(request, parsed);
    }

    #[test]
    fn test_request_serialization_no_to_version() {
        let request = SyncMessage::Request(SyncRequest {
            from_version: 10,
            to_version: None,
        });

        let bytes = request.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();

        assert_eq!(request, parsed);
    }

    #[test]
    fn test_diff_serialization() {
        let diff = SyncMessage::Diff(SyncDiff {
            base_version: 5,
            new_version: 10,
            added: vec![(1, Pattern::new(vec![1, 2, 3]))],
            removed: vec![0],
            hash: 0xDEADBEEF,
        });

        let bytes = diff.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();

        match (&diff, &parsed) {
            (SyncMessage::Diff(d1), SyncMessage::Diff(d2)) => {
                assert_eq!(d1.base_version, d2.base_version);
                assert_eq!(d1.new_version, d2.new_version);
                assert_eq!(d1.added.len(), d2.added.len());
                assert_eq!(d1.removed, d2.removed);
                assert_eq!(d1.hash, d2.hash);
            }
            _ => panic!("Expected Diff messages"),
        }
    }

    #[test]
    fn test_should_announce() {
        let mut sync = Synchronizer::with_config(SyncConfig {
            announce_interval: 3,
            ..Default::default()
        });

        assert!(!sync.should_announce()); // 1
        assert!(!sync.should_announce()); // 2
        assert!(sync.should_announce()); // 3 - triggers
        assert!(!sync.should_announce()); // 1 (reset)
    }

    #[test]
    fn test_sync_timeout() {
        let mut sync = Synchronizer::with_config(SyncConfig {
            sync_timeout: 100,
            ..Default::default()
        });

        sync.state = SyncState::WaitingForSync { requested_at: 0 };

        // Not timed out yet
        assert!(!sync.check_timeout(50));

        // Now timed out
        assert!(sync.check_timeout(150));
        assert_eq!(sync.state, SyncState::Diverged);
    }

    #[test]
    fn test_synchronizer_reset() {
        let mut sync = Synchronizer::new();
        sync.state = SyncState::Diverged;
        sync.local_version = 100;
        sync.remote_version = Some(200);

        sync.reset();

        assert!(sync.is_synchronized());
        assert_eq!(sync.local_version, 0);
        assert!(sync.remote_version.is_none());
    }
}
