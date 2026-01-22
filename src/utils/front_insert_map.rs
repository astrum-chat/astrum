use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

/// A map that maintains insertion order and is optimized for front insertions.
/// Uses a VecDeque for O(1) front insertions while maintaining O(1) key lookups.
#[derive(Clone, Debug)]
pub struct FrontInsertMap<K, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K, V> Default for FrontInsertMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FrontInsertMap<K, V> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<K: Eq + Hash + Clone, V> FrontInsertMap<K, V> {
    /// Insert at the front (O(1) amortized)
    pub fn insert_front(&mut self, key: K, value: V) -> Option<V> {
        let old = self.map.insert(key.clone(), value);
        if old.is_none() {
            self.order.push_front(key);
        }
        old
    }

    /// Insert at the back (O(1) amortized)
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let old = self.map.insert(key.clone(), value);
        if old.is_none() {
            self.order.push_back(key);
        }
        old
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.map.remove(key) {
            self.order.retain(|k| k != key);
            Some(value)
        } else {
            None
        }
    }

    /// Iterate in order (front to back)
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.order
            .iter()
            .filter_map(|k| self.map.get(k).map(|v| (k, v)))
    }

    /// Iterate keys in order
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.order.iter()
    }

    /// Iterate values in order
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.order.iter().filter_map(|k| self.map.get(k))
    }
}
