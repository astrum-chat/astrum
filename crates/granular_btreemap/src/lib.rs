use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
    rc::Rc,
};

pub struct GranularBTreeMap<K, V, O>
where
    K: Eq + Hash,
    O: Ord,
{
    pub lookup_map: HashMap<K, (Rc<O>, Rc<V>)>,
    pub order_map: BTreeMap<Rc<O>, Rc<V>>,
}

impl<K, V, O> Default for GranularBTreeMap<K, V, O>
where
    K: Eq + Hash,
    O: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, O> GranularBTreeMap<K, V, O>
where
    K: Eq + Hash,
    O: Ord,
{
    pub fn new() -> Self {
        Self {
            lookup_map: HashMap::new(),
            order_map: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, key: impl Into<K>, value: impl Into<V>, order_key: impl Into<O>) {
        let key = key.into();
        let value = Rc::new(value.into());
        let order_key = Rc::new(order_key.into());

        // Removes the key from the order_map if it exists.
        if let Some((existing_order_key, _value)) = self.lookup_map.get(&key) {
            self.order_map.remove(&existing_order_key.clone());
        }

        self.lookup_map
            .insert(key.into(), (order_key.clone(), value.clone()));
        self.order_map.insert(order_key.into(), value);
    }

    pub fn update_order_for_key(
        &mut self,
        key: &K,
        new_order_key: impl Into<O>,
    ) -> Result<(), usize> {
        let (key, (order_key, value)) = self.lookup_map.remove_entry(&key).ok_or_else(|| 1usize)?;
        let _ = self.order_map.remove(&order_key).ok_or_else(|| 2usize)?;

        let new_order_key = Rc::new(new_order_key.into());

        self.lookup_map
            .insert(key, (new_order_key.clone(), value.clone()));
        self.order_map.insert(new_order_key, value);

        Ok(())
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.lookup_map
            .get(key)
            .map(|(_order_key, value)| value.as_ref())
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let (order_key, value) = self.lookup_map.remove(key)?;

        self.order_map.remove(&order_key);

        Rc::try_unwrap(value).ok()
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.order_map.values().map(|this| this.as_ref())
    }
}
