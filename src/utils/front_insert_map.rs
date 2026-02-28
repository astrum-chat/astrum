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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let map: FrontInsertMap<String, i32> = FrontInsertMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_with_capacity_is_empty() {
        let map: FrontInsertMap<String, i32> = FrontInsertMap::with_capacity(10);
        assert!(map.is_empty());
    }

    #[test]
    fn test_default_is_empty() {
        let map: FrontInsertMap<String, i32> = FrontInsertMap::default();
        assert!(map.is_empty());
    }

    #[test]
    fn test_insert_back_new_key() {
        let mut map = FrontInsertMap::new();
        let old = map.insert("a", 1);
        assert!(old.is_none());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&"a"), Some(&1));
    }

    #[test]
    fn test_insert_back_duplicate_key_returns_old_value() {
        let mut map = FrontInsertMap::new();
        map.insert("a", 1);
        let old = map.insert("a", 2);
        assert_eq!(old, Some(1));
        assert_eq!(map.get(&"a"), Some(&2));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_insert_back_preserves_order() {
        let mut map = FrontInsertMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        map.insert("c", 3);
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys, vec![&"a", &"b", &"c"]);
    }

    #[test]
    fn test_insert_front_new_key() {
        let mut map = FrontInsertMap::new();
        let old = map.insert_front("a", 1);
        assert!(old.is_none());
        assert_eq!(map.get(&"a"), Some(&1));
    }

    #[test]
    fn test_insert_front_pushes_to_front() {
        let mut map = FrontInsertMap::new();
        map.insert_front("a", 1);
        map.insert_front("b", 2);
        map.insert_front("c", 3);
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys, vec![&"c", &"b", &"a"]);
    }

    #[test]
    fn test_insert_front_duplicate_updates_value_preserves_position() {
        let mut map = FrontInsertMap::new();
        map.insert_front("a", 1);
        map.insert_front("b", 2);
        let old = map.insert_front("a", 10);
        assert_eq!(old, Some(1));
        assert_eq!(map.get(&"a"), Some(&10));
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys, vec![&"b", &"a"]);
    }

    #[test]
    fn test_mixed_insert_front_and_back_ordering() {
        let mut map = FrontInsertMap::new();
        map.insert("x", 1);
        map.insert_front("y", 2);
        map.insert("z", 3);
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys, vec![&"y", &"x", &"z"]);
    }

    #[test]
    fn test_remove_existing_key() {
        let mut map = FrontInsertMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        let removed = map.remove(&"a");
        assert_eq!(removed, Some(1));
        assert_eq!(map.len(), 1);
        assert!(!map.contains_key(&"a"));
    }

    #[test]
    fn test_remove_nonexistent_key() {
        let mut map = FrontInsertMap::new();
        map.insert("a", 1);
        let removed = map.remove(&"z");
        assert!(removed.is_none());
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_get_mut_modifies_value() {
        let mut map = FrontInsertMap::new();
        map.insert("a", 1);
        if let Some(v) = map.get_mut(&"a") {
            *v = 42;
        }
        assert_eq!(map.get(&"a"), Some(&42));
    }

    #[test]
    fn test_iter_yields_pairs_in_order() {
        let mut map = FrontInsertMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        let pairs: Vec<_> = map.iter().collect();
        assert_eq!(pairs, vec![(&"a", &1), (&"b", &2)]);
    }

    #[test]
    fn test_values_in_order() {
        let mut map = FrontInsertMap::new();
        map.insert_front("a", 10);
        map.insert_front("b", 20);
        let vals: Vec<_> = map.values().collect();
        assert_eq!(vals, vec![&20, &10]);
    }
}
