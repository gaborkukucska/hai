//!<!-- # START OF FILE hainet-core/src/storage/crdt.rs -->
//! Conflict-Free Replicated Data Types (CRDTs)
//!
//! Provides data structures that can be replicated across multiple nodes
//! without coordination, automatically resolving conflicts through
//! mathematical properties.
//!
//! ## Constitutional Compliance
//! - Article III (Decentralization): No central authority needed for conflict resolution
//! - Article I (Privacy First): State remains local-first, syncs peer-to-peer

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::SystemTime;

/// Logical timestamp for causality tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp {
    /// Lamport timestamp
    pub logical: u64,
    /// Physical timestamp for tie-breaking
    pub physical: SystemTime,
}

impl Timestamp {
    /// Create new timestamp with current time
    pub fn now(logical: u64) -> Self {
        Self {
            logical,
            physical: SystemTime::now(),
        }
    }

    /// Create timestamp from components
    pub fn new(logical: u64, physical: SystemTime) -> Self {
        Self { logical, physical }
    }

    /// Get next timestamp (increment logical clock)
    pub fn next(&self) -> Self {
        Self {
            logical: self.logical + 1,
            physical: SystemTime::now(),
        }
    }

    /// Merge timestamps (take max of both clocks)
    pub fn merge(&self, other: &Timestamp) -> Self {
        Self {
            logical: self.logical.max(other.logical) + 1,
            physical: SystemTime::now(),
        }
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare logical clocks first
        match self.logical.cmp(&other.logical) {
            Ordering::Equal => {
                // Tie-break with physical timestamp
                self.physical.cmp(&other.physical)
            }
            ord => ord,
        }
    }
}

/// Node identifier for vector clocks
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Vector clock for tracking causality across nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorClock {
    clocks: HashMap<NodeId, u64>,
}

impl VectorClock {
    /// Create new empty vector clock
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    /// Increment clock for given node
    pub fn increment(&mut self, node: &NodeId) {
        *self.clocks.entry(node.clone()).or_insert(0) += 1;
    }

    /// Get clock value for node
    pub fn get(&self, node: &NodeId) -> u64 {
        self.clocks.get(node).copied().unwrap_or(0)
    }

    /// Merge with another vector clock (take max of all clocks)
    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &clock) in &other.clocks {
            let entry = self.clocks.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(clock);
        }
    }

    /// Check if this clock happened before other (a < b)
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut strictly_less = false;
        
        // Check all nodes in both clocks
        let all_nodes: HashSet<_> = self.clocks.keys()
            .chain(other.clocks.keys())
            .collect();

        for node in all_nodes {
            let self_clock = self.get(node);
            let other_clock = other.get(node);

            if self_clock > other_clock {
                return false; // Not happened-before
            }
            if self_clock < other_clock {
                strictly_less = true;
            }
        }

        strictly_less
    }

    /// Check if clocks are concurrent (neither happened before the other)
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self)
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Last-Writer-Wins Register (LWW-Register)
///
/// Simple CRDT that keeps the value with the highest timestamp.
/// Useful for single-value replication with automatic conflict resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T: Clone> {
    value: T,
    timestamp: Timestamp,
    node_id: NodeId,
}

impl<T: Clone> LWWRegister<T> {
    /// Create new register with initial value
    pub fn new(value: T, node_id: NodeId) -> Self {
        Self {
            value,
            timestamp: Timestamp::now(0),
            node_id,
        }
    }

    /// Create register with specific timestamp
    pub fn with_timestamp(value: T, timestamp: Timestamp, node_id: NodeId) -> Self {
        Self {
            value,
            timestamp,
            node_id,
        }
    }

    /// Update value (increments logical clock)
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.timestamp = self.timestamp.next();
    }

    /// Get current value
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Get timestamp
    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    /// Merge with another register (take value with higher timestamp)
    pub fn merge(&mut self, other: &Self) {
        if other.timestamp > self.timestamp {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.node_id = other.node_id.clone();
        } else if other.timestamp == self.timestamp {
            // Tie-break by node ID (deterministic)
            if other.node_id.0 > self.node_id.0 {
                self.value = other.value.clone();
                self.node_id = other.node_id.clone();
            }
        }
    }
}

/// Grow-Only Set (G-Set)
///
/// CRDT set that only supports additions (no removals).
/// Elements can be added concurrently without conflicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSet<T: Clone + Eq + std::hash::Hash> {
    elements: HashSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash> GSet<T> {
    /// Create new empty G-Set
    pub fn new() -> Self {
        Self {
            elements: HashSet::new(),
        }
    }

    /// Add element to set
    pub fn insert(&mut self, element: T) {
        self.elements.insert(element);
    }

    /// Check if element exists
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Get all elements
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }

    /// Get set size
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Merge with another G-Set (union)
    pub fn merge(&mut self, other: &Self) {
        self.elements.extend(other.elements.iter().cloned());
    }
}

impl<T: Clone + Eq + std::hash::Hash> Default for GSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Two-Phase Set (2P-Set)
///
/// CRDT set that supports both additions and removals.
/// Once an element is removed, it cannot be re-added.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoPhaseSet<T: Clone + Eq + std::hash::Hash> {
    added: HashSet<T>,
    removed: HashSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash> TwoPhaseSet<T> {
    /// Create new empty 2P-Set
    pub fn new() -> Self {
        Self {
            added: HashSet::new(),
            removed: HashSet::new(),
        }
    }

    /// Add element to set
    pub fn insert(&mut self, element: T) -> Result<()> {
        if self.removed.contains(&element) {
            anyhow::bail!("Cannot re-add removed element");
        }
        self.added.insert(element);
        Ok(())
    }

    /// Remove element from set
    pub fn remove(&mut self, element: T) -> Result<()> {
        if !self.added.contains(&element) {
            anyhow::bail!("Cannot remove element that was never added");
        }
        self.removed.insert(element);
        Ok(())
    }

    /// Check if element exists (added but not removed)
    pub fn contains(&self, element: &T) -> bool {
        self.added.contains(element) && !self.removed.contains(element)
    }

    /// Get all active elements
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.added.difference(&self.removed)
    }

    /// Get set size
    pub fn len(&self) -> usize {
        self.added.difference(&self.removed).count()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge with another 2P-Set (union of both added and removed)
    pub fn merge(&mut self, other: &Self) {
        self.added.extend(other.added.iter().cloned());
        self.removed.extend(other.removed.iter().cloned());
    }
}

impl<T: Clone + Eq + std::hash::Hash> Default for TwoPhaseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// LWW-Element-Set
///
/// CRDT set combining Last-Writer-Wins strategy with add/remove operations.
/// Elements track timestamps for both additions and removals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWElementSet<T: Clone + Eq + std::hash::Hash> {
    added: HashMap<T, Timestamp>,
    removed: HashMap<T, Timestamp>,
}

impl<T: Clone + Eq + std::hash::Hash> LWWElementSet<T> {
    /// Create new empty LWW-Element-Set
    pub fn new() -> Self {
        Self {
            added: HashMap::new(),
            removed: HashMap::new(),
        }
    }

    /// Add element with current timestamp
    pub fn insert(&mut self, element: T, timestamp: Timestamp) {
        self.added.insert(element, timestamp);
    }

    /// Remove element with current timestamp
    pub fn remove(&mut self, element: T, timestamp: Timestamp) {
        self.removed.insert(element, timestamp);
    }

    /// Check if element exists (add timestamp > remove timestamp)
    pub fn contains(&self, element: &T) -> bool {
        match (self.added.get(element), self.removed.get(element)) {
            (Some(add_ts), Some(rem_ts)) => add_ts > rem_ts,
            (Some(_), None) => true,
            _ => false,
        }
    }

    /// Get all active elements
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.added.keys().filter(move |k| self.contains(k))
    }

    /// Get set size
    pub fn len(&self) -> usize {
        self.added.keys().filter(|k| self.contains(k)).count()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge with another LWW-Element-Set (take max timestamp for each element)
    pub fn merge(&mut self, other: &Self) {
        for (element, timestamp) in &other.added {
            self.added
                .entry(element.clone())
                .and_modify(|ts| *ts = (*ts).max(*timestamp))
                .or_insert(*timestamp);
        }

        for (element, timestamp) in &other.removed {
            self.removed
                .entry(element.clone())
                .and_modify(|ts| *ts = (*ts).max(*timestamp))
                .or_insert(*timestamp);
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash> Default for LWWElementSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_ordering() {
        let ts1 = Timestamp::now(1);
        let ts2 = Timestamp::now(2);
        assert!(ts1 < ts2);
    }

    #[test]
    fn test_timestamp_merge() {
        let ts1 = Timestamp::now(5);
        let ts2 = Timestamp::now(3);
        let merged = ts1.merge(&ts2);
        assert_eq!(merged.logical, 6); // max(5, 3) + 1
    }

    #[test]
    fn test_vector_clock_increment() {
        let mut vc = VectorClock::new();
        let node = NodeId::new("node1");
        
        vc.increment(&node);
        assert_eq!(vc.get(&node), 1);
        
        vc.increment(&node);
        assert_eq!(vc.get(&node), 2);
    }

    #[test]
    fn test_vector_clock_merge() {
        let node1 = NodeId::new("node1");
        let node2 = NodeId::new("node2");

        let mut vc1 = VectorClock::new();
        vc1.increment(&node1);
        vc1.increment(&node1);

        let mut vc2 = VectorClock::new();
        vc2.increment(&node2);

        vc1.merge(&vc2);
        assert_eq!(vc1.get(&node1), 2);
        assert_eq!(vc1.get(&node2), 1);
    }

    #[test]
    fn test_vector_clock_happens_before() {
        let node = NodeId::new("node1");

        let mut vc1 = VectorClock::new();
        vc1.increment(&node);

        let mut vc2 = VectorClock::new();
        vc2.increment(&node);
        vc2.increment(&node);

        assert!(vc1.happens_before(&vc2));
        assert!(!vc2.happens_before(&vc1));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let node1 = NodeId::new("node1");
        let node2 = NodeId::new("node2");

        let mut vc1 = VectorClock::new();
        vc1.increment(&node1);

        let mut vc2 = VectorClock::new();
        vc2.increment(&node2);

        assert!(vc1.is_concurrent(&vc2));
        assert!(vc2.is_concurrent(&vc1));
    }

    #[test]
    fn test_lww_register_set() {
        let node = NodeId::new("node1");
        let mut reg = LWWRegister::new(42, node);
        
        assert_eq!(*reg.get(), 42);
        
        reg.set(100);
        assert_eq!(*reg.get(), 100);
    }

    #[test]
    fn test_lww_register_merge() {
        let node1 = NodeId::new("node1");
        let node2 = NodeId::new("node2");

        let mut reg1 = LWWRegister::new(10, node1.clone());
        let mut reg2 = LWWRegister::new(20, node2.clone());

        // reg2 has higher timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));
        reg2.set(25);

        reg1.merge(&reg2);
        assert_eq!(*reg1.get(), 25);
    }

    #[test]
    fn test_gset_insert() {
        let mut set = GSet::new();
        set.insert("hello");
        set.insert("world");
        
        assert_eq!(set.len(), 2);
        assert!(set.contains(&"hello"));
        assert!(set.contains(&"world"));
    }

    #[test]
    fn test_gset_merge() {
        let mut set1 = GSet::new();
        set1.insert("a");
        set1.insert("b");

        let mut set2 = GSet::new();
        set2.insert("b");
        set2.insert("c");

        set1.merge(&set2);
        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&"a"));
        assert!(set1.contains(&"b"));
        assert!(set1.contains(&"c"));
    }

    #[test]
    fn test_two_phase_set() {
        let mut set = TwoPhaseSet::new();
        
        set.insert("item1").unwrap();
        assert!(set.contains(&"item1"));
        
        set.remove("item1").unwrap();
        assert!(!set.contains(&"item1"));
        
        // Cannot re-add
        assert!(set.insert("item1").is_err());
    }

    #[test]
    fn test_two_phase_set_merge() {
        let mut set1 = TwoPhaseSet::new();
        set1.insert("a").unwrap();
        set1.insert("b").unwrap();

        let mut set2 = TwoPhaseSet::new();
        set2.insert("b").unwrap();
        set2.insert("c").unwrap();
        set2.remove("b").unwrap();

        set1.merge(&set2);
        assert!(set1.contains(&"a"));
        assert!(!set1.contains(&"b")); // Removed in set2
        assert!(set1.contains(&"c"));
    }

    #[test]
    fn test_lww_element_set() {
        let mut set = LWWElementSet::new();
        let ts1 = Timestamp::now(1);
        let ts2 = Timestamp::now(2);

        set.insert("item1", ts1);
        assert!(set.contains(&"item1"));

        set.remove("item1", ts2);
        assert!(!set.contains(&"item1"));
    }

    #[test]
    fn test_lww_element_set_concurrent_add_remove() {
        let mut set = LWWElementSet::new();
        let ts = Timestamp::now(1);

        // Add and remove at same timestamp (add wins by convention)
        set.insert("item", ts);
        set.remove("item", ts);
        
        // Should not contain (remove timestamp == add timestamp)
        assert!(!set.contains(&"item"));
    }

    #[test]
    fn test_lww_element_set_merge() {
        let ts1 = Timestamp::now(1);
        let ts2 = Timestamp::now(2);

        let mut set1 = LWWElementSet::new();
        set1.insert("a", ts1);

        let mut set2 = LWWElementSet::new();
        set2.insert("b", ts2);
        set2.remove("a", ts2);

        set1.merge(&set2);
        assert!(!set1.contains(&"a")); // Removed with higher timestamp
        assert!(set1.contains(&"b"));
    }
}
