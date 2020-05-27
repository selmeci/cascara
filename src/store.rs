use crate::cache::OnEvict;
use crate::tiny_lfu::TinyLFU;
use crate::ttl::{Expiration, ExpirationMap};
use indexmap::map::IndexMap;
use log::warn;
use rand::distributions::Uniform;
use rand::{thread_rng, Rng};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::time::{Duration, SystemTime};

///
/// How many samples are check to find one with lowest estimate
///
pub const SAMPLES_NUM: usize = 5;

///
/// SampleItem hold info about item and its estimate in TinyLFU
///
#[derive(Clone, Debug)]
pub struct SampleItem {
    ///
    /// Item identification
    ///
    pub key: u64,

    ///
    /// Item estimate from TinyFLF
    ///
    pub estimate: i64,
}

impl SampleItem {
    ///
    /// New Sample item
    ///
    /// # Arguments
    ///
    /// - `key`: item identification
    /// - `estimate`: item estimate
    ///
    pub fn new(key: u64, estimate: i64) -> Self {
        Self { key, estimate }
    }
}

impl PartialOrd for SampleItem {
    fn partial_cmp(&self, other: &SampleItem) -> Option<Ordering> {
        self.estimate
            .partial_cmp(&other.estimate)
            .map(|ord| ord.then(self.key.cmp(&other.key)))
    }
}

impl Ord for SampleItem {
    fn cmp(&self, other: &SampleItem) -> Ordering {
        self.estimate
            .cmp(&other.estimate)
            .then(self.key.cmp(&other.key))
    }
}

impl PartialEq for SampleItem {
    fn eq(&self, other: &Self) -> bool {
        self.key.eq(&other.key)
    }
}

impl Eq for SampleItem {}

impl Hash for SampleItem {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

///
/// Storage item with hold item key, value and optionally expiration time
///
#[derive(Debug)]
pub struct Item<K, V> {
    ///
    /// Expiration time
    ///
    pub expiration_time: Option<SystemTime>,

    ///
    /// Item key
    ///
    pub k: K,

    ///
    /// Item value
    ///
    pub v: V,
}

impl<K, V> Item<K, V> {
    ///
    /// New storage item without expiration time
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    /// - `v`: item value
    ///
    pub fn new(k: K, v: V) -> Self {
        Self {
            expiration_time: None,
            k,
            v,
        }
    }
}

impl<K, V> Deref for Item<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}

///
/// Storage supported functions
///
pub trait Store<K, V>: Iterator {
    ///
    /// Returns how many items can be hold in storage
    ///
    fn capacity(&self) -> usize;

    ///
    /// Returns actual number of items in storage
    ///
    fn len(&self) -> usize;

    ///
    /// Returns true if storage is empty
    ///
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    ///
    /// Returns how many room left
    ///
    fn room_left(&self) -> usize;

    ///
    /// Return true if item is in storage
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    ///
    fn contains(&self, k: &u64) -> bool;

    ///
    /// Return item ref if is in storage
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    ///
    fn get(&self, k: &u64) -> Option<&Item<K, V>>;

    ///
    /// Return mutable item ref if is in storage
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    ///
    fn get_mut(&mut self, k: &u64) -> Option<&mut Item<K, V>>;

    ///
    /// Insert item into storage. Returns preview item if exists with given key.
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    /// - `item`: storage item
    ///
    fn insert(&mut self, k: u64, item: Item<K, V>) -> Option<Item<K, V>> {
        self.insert_with_ttl(k, item, Duration::from_secs(0))
    }

    ///
    /// Insert item into storage with defined time to life in seconds. Returns preview item if exists with given key.
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    /// - `item`: storage item
    /// - `expiration`: how many seconds should item lives
    ///
    fn insert_with_ttl(
        &mut self,
        k: u64,
        item: Item<K, V>,
        expiration: Duration,
    ) -> Option<Item<K, V>>;

    ///
    /// Remove and return item from storage.
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    ///
    fn remove(&mut self, k: &u64) -> Option<Item<K, V>>;

    ///
    /// Remove all expired items from storage.
    /// Call `on_evict` for every removed item.
    ///
    fn cleanup<E>(&mut self, on_evict: &Option<E>)
    where
        E: OnEvict<K, V>;

    ///
    /// Remove all items from storage.
    ///
    fn clear(&mut self);

    ///
    /// If storage contains any items, than return one item with lowest estimate from checked sample.
    ///
    /// # Arguments
    ///
    /// - `admit`: TinyLFU for calculating estimate of item in storage.
    ///
    fn sample(&self, admit: &impl TinyLFU) -> Option<SampleItem>;
}

///
/// Basic implementation of storage for cache.
///
/// Data are hold in IndexMap which allow to create sample set.
///
pub struct Storage<K, V> {
    data: IndexMap<u64, Item<K, V>>,
    expiration_map: ExpirationMap,
    capacity: usize,
}

impl<K, V> Storage<K, V> {
    ///
    /// Create new storage with defined capacity.
    ///
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            data: IndexMap::new(),
            expiration_map: ExpirationMap::new(),
        }
    }
}

impl<K, V> Iterator for Storage<K, V> {
    type Item = Item<K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        unimplemented!()
    }
}

impl<K, V> Store<K, V> for Storage<K, V> {
    fn capacity(&self) -> usize {
        self.capacity
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn room_left(&self) -> usize {
        self.capacity() - self.len()
    }

    fn contains(&self, k: &u64) -> bool {
        self.get(k).is_some()
    }

    fn get(&self, k: &u64) -> Option<&Item<K, V>> {
        if let Some(item) = self.data.get(k) {
            if let Some(expiration_time) = &item.expiration_time {
                if SystemTime::now().gt(expiration_time) {
                    None
                } else {
                    Some(item)
                }
            } else {
                Some(item)
            }
        } else {
            None
        }
    }

    fn get_mut(&mut self, k: &u64) -> Option<&mut Item<K, V>> {
        if let Some(item) = self.data.get_mut(k) {
            if let Some(expiration_time) = &item.expiration_time {
                if SystemTime::now().gt(expiration_time) {
                    None
                } else {
                    Some(item)
                }
            } else {
                Some(item)
            }
        } else {
            None
        }
    }

    fn insert_with_ttl(
        &mut self,
        k: u64,
        mut item: Item<K, V>,
        expiration: Duration,
    ) -> Option<Item<K, V>> {
        let old_item = if let Some(old_item) = self.data.remove(&k) {
            if let Some(expiration_time) = &old_item.expiration_time {
                self.expiration_map.remove(&k, expiration_time);
            }
            Some(old_item)
        } else {
            None
        };
        item.expiration_time = self.expiration_map.insert(k, expiration);
        self.data.insert(k, item);
        old_item
    }

    fn remove(&mut self, k: &u64) -> Option<Item<K, V>> {
        if let Some(item) = self.data.remove(k) {
            if let Some(expiration_time) = &item.expiration_time {
                self.expiration_map.remove(k, expiration_time);
            }
            Some(item)
        } else {
            None
        }
    }

    fn cleanup<E>(&mut self, on_evict: &Option<E>)
    where
        E: OnEvict<K, V>,
    {
        let now = SystemTime::now();
        let keys = self.expiration_map.cleanup(&now);
        for k in keys {
            if let Some(item) = self.data.get(&k) {
                if let Some(expiration_time) = &item.expiration_time {
                    if now.lt(expiration_time) {
                        warn!("Expiration map contains invalid expiration time for item!");
                        continue;
                    }
                } else {
                    warn!("Expiration map contains item without expiration time!");
                    continue;
                }
            } else {
                warn!("Expiration map contains invalid item!");
                continue;
            }
            let item = self.remove(&k).unwrap();
            if let Some(on_evict) = on_evict {
                on_evict.evict(&item.k, &item.v);
            }
        }
    }

    fn clear(&mut self) {
        self.expiration_map.clear();
        self.data.clear();
    }

    fn sample(&self, admit: &impl TinyLFU) -> Option<SampleItem> {
        if self.is_empty() {
            return None;
        }
        let items_range = Uniform::new(0_usize, self.len());
        let mut generator = thread_rng().sample_iter(items_range);
        let mut result: Option<SampleItem> = None;
        for _ in 0..SAMPLES_NUM {
            let index = generator.next().unwrap();
            let (k, _) = self.data.get_index(index).expect("sample item");
            let estimate = admit.estimate(&k);
            let sample = SampleItem::new(*k, estimate);
            if let Some(current) = &result {
                if sample.estimate.lt(&current.estimate) {
                    result = Some(sample);
                }
            } else {
                result = Some(sample)
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::OnEvict;
    use crate::store::{Item, SampleItem, Storage, Store};
    use crate::tiny_lfu::{TinyLFU, TinyLFUCache};
    use std::cmp::Ordering;
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};
    use std::ops::Deref;
    use std::time::Duration;

    struct Evict {}

    impl<K, V> OnEvict<K, V> for Evict {
        fn evict(&self, _k: &K, _v: &V) {}
    }

    #[test]
    fn sample_partial_cmp() {
        let item_1 = SampleItem::new(1, 2);
        let item_2 = SampleItem::new(2, 1);
        let item_3 = SampleItem::new(1, 2);
        assert_eq!(item_1.partial_cmp(&item_2), Some(Ordering::Greater));
        assert_eq!(item_2.partial_cmp(&item_1), Some(Ordering::Less));
        assert_eq!(item_1.partial_cmp(&item_3), Some(Ordering::Equal));
    }

    #[test]
    fn sample_cmp() {
        let item_1 = SampleItem::new(1, 2);
        let item_2 = SampleItem::new(2, 1);
        let item_3 = SampleItem::new(1, 2);
        assert_eq!(item_1.cmp(&item_2), Ordering::Greater);
        assert_eq!(item_2.cmp(&item_1), Ordering::Less);
        assert_eq!(item_1.cmp(&item_3), Ordering::Equal);
    }

    #[test]
    fn sample_eq() {
        let item_1 = SampleItem::new(1, 2);
        let item_2 = SampleItem::new(2, 1);
        let item_3 = SampleItem::new(1, 2);
        assert_eq!(item_1.eq(&item_2), false);
        assert_eq!(item_2.eq(&item_1), false);
        assert_eq!(item_1.eq(&item_3), true);
    }

    #[test]
    fn hash() {
        let item_1 = SampleItem::new(1, 2);
        let mut state = RandomState::new().build_hasher();
        item_1.hash(&mut state);
        let _hash = state.finish();
        assert!(true);
    }

    #[test]
    fn deref() {
        let item_1 = Item::new(1, 2);
        let v = item_1.deref();
        assert_eq!(v, &2);
    }

    #[test]
    fn do_not_get_expired_items() {
        let mut store = Storage::with_capacity(10);
        assert!(store
            .insert_with_ttl(1, Item::new(1, 2), Duration::from_secs(1))
            .is_none());
        std::thread::sleep(Duration::from_secs(2));
        assert!(store.get(&1).is_none());
    }

    #[test]
    fn get_not_expired_items() {
        let mut store = Storage::with_capacity(10);
        assert!(store
            .insert_with_ttl(1, Item::new(1, 2), Duration::from_secs(10))
            .is_none());
        assert!(store.get(&1).is_some());
    }

    #[test]
    fn do_not_get_mut_expired_items() {
        let mut store = Storage::with_capacity(10);
        assert!(store
            .insert_with_ttl(1, Item::new(1, 2), Duration::from_secs(1))
            .is_none());
        std::thread::sleep(Duration::from_secs(2));
        assert!(store.get_mut(&1).is_none());
    }

    #[test]
    fn get_mut_not_expired_items() {
        let mut store = Storage::with_capacity(10);
        assert!(store
            .insert_with_ttl(1, Item::new(1, 2), Duration::from_secs(10))
            .is_none());
        assert!(store.get_mut(&1).is_some());
    }

    #[test]
    fn set_and_get() {
        let mut store = Storage::with_capacity(10);
        assert!(store.insert(1, Item::new(1, 2)).is_none());
        let item = store.get(&1);
        assert!(item.is_some());
        if let Some(item) = item {
            assert!(item.k.eq(&1));
            assert!(item.v.eq(&2));
        }
        assert!(store.insert(1, Item::new(1, 3)).is_some());
        let item = store.get(&1);
        assert!(item.is_some());
        if let Some(item) = item {
            assert!(item.k.eq(&1));
            assert!(item.v.eq(&3));
        }
        assert!(store.get(&2).is_none());
        assert!(store.get_mut(&2).is_none());
        assert!(store.insert(2, Item::new(2, 4)).is_none());
        let item = store.get(&2);
        assert!(item.is_some());
        if let Some(item) = item {
            assert!(item.k.eq(&2));
            assert!(item.v.eq(&4));
        }
    }

    #[test]
    fn remove() {
        let mut store = Storage::with_capacity(10);
        assert!(store.insert(1, Item::new(1, 2)).is_none());
        let item = store.remove(&1);
        assert!(item.is_some());
        if let Some(item) = item {
            assert_eq!(item.k, 1);
            assert_eq!(item.v, 2);
        }
        assert!(store.remove(&1).is_none());
    }

    #[test]
    fn clear() {
        let mut store = Storage::with_capacity(10);
        for i in 1..1000 {
            assert!(store.insert(i, Item::new(i, i)).is_none())
        }
        store.clear();
        for i in 1..1000 {
            assert!(store.get(&i).is_none())
        }
    }

    #[test]
    fn get_mut() {
        let mut store = Storage::with_capacity(10);
        assert!(store.insert(1, Item::new(1, 2)).is_none());
        let item = store.get_mut(&1);
        assert!(item.is_some());
        if let Some(item) = item {
            item.v = 3;
        }
        let item = store.get(&1);
        assert!(item.is_some());
        if let Some(item) = item {
            assert_eq!(item.v, 3);
        }
    }

    #[test]
    fn room_left() {
        let mut store = Storage::with_capacity(10);
        for i in 0..3 {
            store.insert(i, Item::new(i, i));
        }
        assert_eq!(store.room_left(), 7);
    }

    #[test]
    fn some_sample() {
        let mut store = Storage::with_capacity(10);
        for i in 0..10 {
            store.insert(i, Item::new(i, i));
        }
        let admit = TinyLFUCache::new(16);
        let sample = store.sample(&admit);
        assert!(sample.is_some());
    }

    #[test]
    fn no_sample() {
        let store = Storage::<u64, u64>::with_capacity(10);
        let admit = TinyLFUCache::new(16);
        let sample = store.sample(&admit);
        assert!(sample.is_none());
    }

    #[test]
    fn min_sample() {
        let mut store = Storage::<u64, u64>::with_capacity(10);
        for i in 0..2 {
            store.insert(i, Item::new(i, i));
        }
        let mut admit = TinyLFUCache::new(16);
        admit.increment(&0);
        let sample = store.sample(&admit);
        assert!(sample.is_some());
        if let Some(sample) = sample {
            assert_eq!(sample.key, 1);
        }
    }

    #[test]
    fn cleanup() {
        let mut store = Storage::<u64, u64>::with_capacity(10);
        for i in 0..2 {
            store.insert(i, Item::new(i, i));
        }
        for i in 2..4 {
            store.insert_with_ttl(i, Item::new(i, i), Duration::from_secs(1));
        }
        std::thread::sleep(Duration::from_secs(2));
        let on_evict = Some(Evict {});
        store.cleanup(&on_evict);
        assert!(store.contains(&0));
        assert!(store.contains(&1));
        assert!(!store.contains(&2));
        assert!(!store.contains(&3));
    }

    #[test]
    fn update_with_ttl() {
        let mut store = Storage::<u64, u64>::with_capacity(10);
        assert!(store
            .insert_with_ttl(1, Item::new(1, 1), Duration::from_secs(2))
            .is_none());
        std::thread::sleep(Duration::from_secs(1));
        assert!(store.contains(&1));
        assert!(store
            .insert_with_ttl(1, Item::new(1, 1), Duration::from_secs(3))
            .is_some());
        std::thread::sleep(Duration::from_secs(2));
        store.cleanup::<Evict>(&None);
        assert!(store.contains(&1));
        std::thread::sleep(Duration::from_secs(2));
        assert!(!store.contains(&1));
    }
}
