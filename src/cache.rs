use crate::iter::Iter;
use crate::metrics::{MetricType, Metrics};
use crate::store::{Item, SampleItem, Storage, Store};
use crate::tiny_lfu::{TinyLFU, TinyLFUCache, MAX_WINDOW_SIZE};
use probabilistic_collections::SipHasherBuilder;
use std::hash::{BuildHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Mutex;
use std::time::Duration;

pub trait OnEvict<K, V> {
    fn evict(&self, k: &K, v: &V);
}

pub struct VoidEvict<K, V> {
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<K, V> OnEvict<K, V> for VoidEvict<K, V> {
    fn evict(&self, _k: &K, _v: &V) {}
}

///
/// Default implementation of Cache with TinyLFU admit policy.
///
pub struct Cache<
    K,
    V,
    E = VoidEvict<K, V>,
    S = Storage<K, V>,
    A = TinyLFUCache,
    H = SipHasherBuilder,
> where
    K: Eq + Hash,
    E: OnEvict<K, V>,
    S: Store<K, V>,
    A: TinyLFU,
    H: BuildHasher,
{
    hasher_builder: H,
    pub(crate) store: S,
    admit: Mutex<A>,
    on_evict: Option<E>,
    metrics: Mutex<Option<Metrics>>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<K: Eq + Hash, V> Cache<K, V> {
    ///
    /// Create new cache with default `window_size` = 10000 for TinyLFU.
    ///
    /// # Arguments
    ///
    ///- `capacity`: max items in cache
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let cache = Cache::<u8,u8>::new(100);
    /// ```
    ///
    /// # Panic
    ///
    /// If `capacity` is 0.
    ///
    pub fn new(capacity: usize) -> Self {
        Self::with_window_size(capacity, MAX_WINDOW_SIZE)
    }

    ///
    /// Create new cache with defined window size fo TinyLFU.
    ///
    /// # Arguments
    ///
    /// - `window_size`: window size for TinyLFU (max. 10 000)
    ///- `capacity`: max items in cache
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let cache = Cache::<u8,u8>::with_window_size(100,20);
    /// ```
    ///
    /// # Panic
    ///
    /// If `window_size` or `capacity` is 0.
    /// If `window_size` > 10 000
    ///
    pub fn with_window_size(capacity: usize, window_size: usize) -> Self {
        assert_ne!(window_size, 0);
        assert!(window_size <= 10_000);
        assert_ne!(capacity, 0);
        Self {
            _k: PhantomData::default(),
            _v: PhantomData::default(),
            metrics: Mutex::new(None),
            on_evict: None,
            admit: Mutex::new(TinyLFUCache::new(window_size)),
            store: Storage::with_capacity(capacity),
            hasher_builder: SipHasherBuilder::from_entropy(),
        }
    }
}

impl<K, V, E> Cache<K, V, E>
where
    K: Eq + Hash,
    E: OnEvict<K, V>,
{
    ///
    /// Create new cache with TinyLFU window size = 10 000 and callback for evicted items from cache.
    ///
    /// # Arguments
    ///
    /// - `capacity`: max items in cache
    /// - `on_evict`: will be call for every item evicted from cache.
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::{Cache, OnEvict};
    ///
    /// #[derive(Default)]
    /// struct Evict {}
    ///
    /// impl OnEvict<u8, u8> for Evict {
    ///     fn evict(&self, k: &u8, v: &u8) {
    ///         println!("Evict item.  k={}, v={}", k, v);
    ///     }
    /// }
    ///
    /// let cache = Cache::<u8,u8,Evict>::with_on_evict(100, Evict::default());
    /// ```
    ///
    /// # Panic
    ///
    /// If  `capacity` is 0.
    ///
    pub fn with_on_evict(capacity: usize, on_evict: E) -> Self {
        Self::with_on_evict_and_window_size(capacity, on_evict, MAX_WINDOW_SIZE)
    }

    ///
    /// Create new cache with defined window size for TinyLFU and callback for evicted items from cache.
    ///
    /// # Arguments
    ///
    /// - `capacity`: max items in cache
    /// - `on_evict`: will be call for every item evicted from cache.
    /// - `window_size`: window size for TinyLFU (max. 10 000)
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::{Cache, OnEvict};
    ///
    /// #[derive(Default)]
    /// struct Evict {}
    ///
    /// impl OnEvict<u8, u8> for Evict {
    ///     fn evict(&self, k: &u8, v: &u8) {
    ///         println!("Evict item.  k={}, v={}", k, v);
    ///     }
    /// }
    ///
    /// let cache = Cache::<u8,u8,Evict>::with_on_evict_and_window_size(100, Evict::default(), 20);
    /// ```
    ///
    /// # Panic
    ///
    /// If `window_size` or `capacity` is 0.
    /// If `window_size` > 10 000
    ///
    pub fn with_on_evict_and_window_size(capacity: usize, on_evict: E, window_size: usize) -> Self {
        assert_ne!(window_size, 0);
        assert!(window_size <= 10_000);
        assert_ne!(capacity, 0);
        Self {
            _k: PhantomData::default(),
            _v: PhantomData::default(),
            metrics: Mutex::new(None),
            on_evict: Some(on_evict),
            admit: Mutex::new(TinyLFUCache::new(window_size)),
            store: Storage::with_capacity(capacity),
            hasher_builder: SipHasherBuilder::from_entropy(),
        }
    }
}

impl<K, V, E, S, A, H> Cache<K, V, E, S, A, H>
where
    K: Eq + Hash,
    E: OnEvict<K, V>,
    S: Store<K, V>,
    A: TinyLFU,
    H: BuildHasher,
{
    ///
    /// Calculate hash for given item key
    ///
    fn key_hash(&self, k: &K) -> u64 {
        let mut hasher = self.hasher_builder.build_hasher();
        k.hash(&mut hasher);
        hasher.finish()
    }

    ///
    /// Remove selected victim from storage.
    ///
    fn remove_victim(&mut self, victim: Option<SampleItem>) {
        if let Some(victim) = victim {
            if let Some(removed) = self.store.remove(&victim.key) {
                let k = self.key_hash(&removed.k);
                let mut metrics = self.metrics.lock().unwrap();
                if let Some(metrics) = &mut *metrics {
                    metrics.insert(MetricType::KeyEvict, &k, 1);
                }
                if let Some(on_evict) = &self.on_evict {
                    on_evict.evict(&removed.k, &removed.v);
                }
            }
        }
    }

    ///
    /// Insert item into storage
    ///
    fn insert_item_with_ttl(
        &mut self,
        k: u64,
        item: Item<K, V>,
        expiration: Duration,
    ) -> Option<V> {
        if let Some(old_item) = self.store.insert_with_ttl(k, item, expiration) {
            Some(old_item.v)
        } else {
            None
        }
    }

    ///
    /// Check if item can be inserted.
    ///
    /// Item can be inserted if:
    ///
    /// - there is a room
    /// - incoming item estimate if bigger than sample item from cache
    ///
    /// Insertion check can return victim which should be removed from cache
    ///
    fn can_be_insert(&mut self, k: &u64) -> Result<Option<SampleItem>, Option<SampleItem>> {
        //no need to find victims if already in cache
        if self.store.contains(k) {
            let mut metrics = self.metrics.lock().unwrap();
            if let Some(metrics) = &mut *metrics {
                metrics.insert(MetricType::KeyUpdate, &k, 1);
            }
            return Ok(None);
        }

        //insert item to cache if there is enough space
        if self.store.room_left() > 0 {
            return Ok(None);
        }

        //try find victim and check if incoming item estimate is enough
        let admit = self.admit.lock().unwrap();
        let incoming_estimate = admit.estimate(&k);

        let victim = self.store.sample(&*admit);
        if let Some(victim) = victim {
            if incoming_estimate < victim.estimate {
                Err(Some(victim))
            } else {
                Ok(Some(victim))
            }
        } else {
            unreachable!()
        }
    }

    ///
    /// Activate metric collecting in cache
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let cache = Cache::<u8,u8>::new(100).with_metrics();
    /// assert!(cache.metrics().is_some());
    /// ```
    ///
    pub fn with_metrics(self) -> Self {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.replace(Metrics::new());
        }
        self
    }

    ///
    /// Returns how many items can be hold in cache
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let cache = Cache::<u8,u8>::new(100);
    /// assert_eq!(cache.capacity(), 100);
    /// ```
    ///
    pub fn capacity(&self) -> usize {
        self.store.capacity()
    }

    ///
    /// Returns actual number of items in cache
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert(1,1).is_ok());
    /// assert_eq!(cache.len(), 1);
    /// ```
    ///
    pub fn len(&self) -> usize {
        self.store.len()
    }

    ///
    /// Returns true if cache is empty
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let cache = Cache::<u8,u8>::new(100);
    /// assert!(cache.is_empty());
    /// ```
    ///
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    ///
    /// Returns how many room left
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert_eq!(cache.room_left(), 100);
    /// assert!(cache.insert(1,1).is_ok());
    /// assert_eq!(cache.room_left(), 99);
    /// ```
    ///
    pub fn room_left(&self) -> usize {
        self.store.room_left()
    }

    ///
    /// Return true if item is in storage
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(!cache.contains(&1));
    /// assert!(cache.insert(1,1).is_ok());
    /// assert!(cache.contains(&1));
    /// ```
    ///
    pub fn contains(&self, k: &K) -> bool {
        let k = self.key_hash(k);
        self.store.contains(&k)
    }

    ///
    /// Return item ref if is in cache
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert(1,2).is_ok());
    /// assert_eq!(cache.get(&1), Some(&2));
    /// ```
    ///
    pub fn get(&self, k: &K) -> Option<&V> {
        let k = self.key_hash(k);
        {
            let mut admit = self.admit.lock().unwrap();
            admit.increment(&k);
        }
        let result = if let Some(item) = self.store.get(&k) {
            Some(&item.v)
        } else {
            None
        };
        let found = result.is_some();
        {
            let mut metrics = self.metrics.lock().unwrap();
            if let Some(metrics) = &mut *metrics {
                if found {
                    metrics.insert(MetricType::Hit, &k, 1);
                } else {
                    metrics.insert(MetricType::Miss, &k, 1);
                }
            }
        }
        result
    }

    ///
    /// Return mutable item ref if is in cache
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert(1,1).is_ok());
    /// let v = cache.get_mut(&1);
    /// assert!(v.is_some());
    /// if let Some(v) = v {
    ///     *v = 2;
    /// }
    /// assert_eq!(cache.get(&1), Some(&2));
    /// ```
    ///
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let k = self.key_hash(k);
        {
            let mut admit = self.admit.lock().unwrap();
            admit.increment(&k);
        }
        let result = if let Some(item) = self.store.get_mut(&k) {
            Some(&mut item.v)
        } else {
            None
        };
        let found = result.is_some();
        {
            let mut metrics = self.metrics.lock().unwrap();
            if let Some(metrics) = &mut *metrics {
                if found {
                    metrics.insert(MetricType::Hit, &k, 1);
                } else {
                    metrics.insert(MetricType::Miss, &k, 1);
                }
            }
        }
        result
    }

    ///
    /// Insert item into cache. Item can be rejected (return Err) if  cache is full and estimate of new item is lower than sample item from cache.
    /// If item is inserted, than preview item value can be returned.
    /// Cache is cleaned and all expired items are removed before new is inserted.
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    /// - `v`: item value
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert(1,1).is_ok());
    /// let v = cache.insert(1,2);
    /// assert!(v.is_ok());
    /// if let Ok(v) = v {
    ///      assert_eq!(v, Some(1));
    /// }
    /// ```
    ///
    pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>, Option<()>> {
        self.insert_with_ttl(k, v, Duration::from_secs(0))
    }

    ///
    /// Insert item into cache with defined time to life in seconds.
    /// Returns preview item if exists with given key.
    /// Cache is cleaned and all expired items are removed before new is inserted.
    ///
    /// If expiration time is 0 sec, than item is insert without ttl.
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    /// - `v`: item value
    /// - `expiration`: how many seconds should item lives
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    /// use std::time::Duration;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert_with_ttl(1,1, Duration::from_secs(1)).is_ok());
    /// assert!(cache.contains(&1));
    /// std::thread::sleep(Duration::from_secs(2));
    /// assert!(!cache.contains(&1));
    /// ```
    ///
    pub fn insert_with_ttl(
        &mut self,
        k: K,
        v: V,
        expiration: Duration,
    ) -> Result<Option<V>, Option<()>> {
        self.store.cleanup(&self.on_evict);

        let key_hash = self.key_hash(&k);
        let item = Item::new(k, v);

        match self.can_be_insert(&key_hash) {
            Ok(victim) => {
                {
                    let mut admit = self.admit.lock().unwrap();
                    admit.increment(&key_hash);
                }
                self.remove_victim(victim);
                {
                    let mut metrics = self.metrics.lock().unwrap();
                    if let Some(metrics) = &mut *metrics {
                        metrics.insert(MetricType::KeyInsert, &key_hash, 1);
                    }
                }
                Ok(self.insert_item_with_ttl(key_hash, item, expiration))
            }
            Err(victim) => {
                self.remove_victim(victim);
                Err(Some(()))
            }
        }
    }

    ///
    /// Remove and return item from cache.
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert(1,2).is_ok());
    /// assert!(cache.contains(&1));
    /// assert_eq!(cache.remove(&1), Some(2));
    /// assert!(!cache.contains(&1));
    /// ```
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let k = self.key_hash(k);
        if let Some(item) = self.store.remove(&k) {
            Some(item.v)
        } else {
            None
        }
    }

    ///
    /// Remove all items from cache.
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::new(100);
    /// assert!(cache.insert(1,1).is_ok());
    /// assert!(cache.insert(2,2).is_ok());
    /// assert!(cache.contains(&1));
    /// assert!(cache.contains(&2));
    /// cache.clear();
    /// assert!(!cache.contains(&1));
    /// assert!(!cache.contains(&2));
    /// ```
    ///
    pub fn clear(&mut self) {
        self.store.clear();
        {
            let mut admit = self.admit.lock().unwrap();
            admit.clear();
        }
        {
            let mut metrics = self.metrics.lock().unwrap();
            if let Some(metrics) = &mut *metrics {
                metrics.clear();
            }
        }
    }

    ///
    /// Return cache metrics
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let cache = Cache::<u8,u8>::new(100).with_metrics();
    /// assert!(cache.metrics().is_some());
    /// ```
    ///
    pub fn metrics(&self) -> Option<Metrics> {
        let metrics = self.metrics.lock().unwrap();
        if let Some(metrics) = &*metrics {
            Some(metrics.clone())
        } else {
            None
        }
    }

    ///
    /// An iterator visiting all entries in order. The iterator element type is (&'a K, &'a V).
    ///
    /// # Example
    ///
    /// ```
    /// use cascara::Cache;
    ///
    /// let mut cache = Cache::<u8, u8>::new(10);
    /// assert!(cache.insert(1, 1).is_ok());
    /// assert!(cache.insert(2, 2).is_ok());
    /// assert!(cache.insert(3, 3).is_ok());
    /// for (key, val) in cache.iter() {
    ///     println!("key: {} val: {}", key, val);
    /// }
    /// ```
    ///
    pub fn iter(&self) -> Iter<K, V, S> {
        Iter::new(&self.store)
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::{Cache, OnEvict};
    use crate::tiny_lfu::TinyLFU;
    use std::fmt::Debug;
    use std::time::Duration;

    #[test]
    fn estimate() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert!(cache.insert(2, 2).is_ok());
        assert!(cache.insert(2, 2).is_ok());
        assert_eq!(cache.admit.lock().unwrap().estimate(&cache.key_hash(&1)), 1);
        assert_eq!(cache.admit.lock().unwrap().estimate(&cache.key_hash(&2)), 2);
    }

    #[test]
    fn insert() {
        let mut cache = Cache::new(2).with_metrics();
        if let Ok(preview) = cache.insert(1, 1) {
            assert!(preview.is_none());
        } else {
            assert!(false, "Item should inserted");
        }
        assert!(cache.contains(&1));
    }

    #[test]
    fn insert_with_ttl() {
        let mut cache = Cache::new(2).with_metrics();
        if let Ok(preview) = cache.insert_with_ttl(1, 1, Duration::from_secs(1)) {
            assert!(preview.is_none());
        } else {
            assert!(false, "Item should inserted");
        }
        assert!(cache.contains(&1));
    }

    #[test]
    fn cleanup_before_insert() {
        let mut cache = Cache::new(2).with_metrics();
        assert!(cache.insert_with_ttl(1, 1, Duration::from_secs(1)).is_ok());
        assert!(cache.contains(&1));
        std::thread::sleep(Duration::from_secs(2));
        assert!(cache.insert(2, 2).is_ok());
        assert!(!cache.contains(&1));
        assert!(cache.contains(&2));
    }

    #[derive(Default, Debug)]
    struct TestEvict {}

    impl OnEvict<usize, usize> for TestEvict {
        fn evict(&self, k: &usize, v: &usize) {
            assert_eq!(k, &1);
            assert_eq!(v, &2);
        }
    }

    #[test]
    fn cleanup_with_evict() {
        let mut cache = Cache::with_on_evict(2, TestEvict::default()).with_metrics();
        assert!(cache.insert_with_ttl(1, 2, Duration::from_secs(1)).is_ok());
        assert!(cache.contains(&1));
        std::thread::sleep(Duration::from_secs(2));
        assert!(cache.insert(2, 2).is_ok());
        assert!(!cache.contains(&1));
        assert!(cache.contains(&2));
    }

    #[test]
    fn update() {
        let mut cache = Cache::new(2).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        if let Ok(preview) = cache.insert(1, 2) {
            assert!(preview.is_some());
            if let Some(preview) = preview {
                assert_eq!(preview, 1);
            }
            assert_eq!(cache.admit.lock().unwrap().estimate(&cache.key_hash(&1)), 2);
        } else {
            assert!(false, "Item should be in cache");
        }
        assert!(cache.contains(&1));
    }

    #[test]
    fn insert_without_victim() {
        let mut cache = Cache::new(2).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert!(cache.insert(2, 2).is_ok());
        assert!(cache.contains(&1));
        assert!(cache.contains(&2));
    }

    #[test]
    fn insert_with_victim() {
        let mut cache = Cache::with_on_evict(2, TestEvict::default()).with_metrics();
        assert!(cache.insert(1, 2).is_ok());
        assert!(cache.insert(2, 2).is_ok());
        let k = cache.key_hash(&2);
        cache.admit.get_mut().unwrap().increment(&k);
        let k = cache.key_hash(&3);
        cache.admit.get_mut().unwrap().increment(&k);
        assert!(cache.insert(3, 3).is_ok());
        assert!(cache.contains(&2));
        assert!(cache.contains(&3));
        assert!(!cache.contains(&1), "Victim should be value 1");
    }

    #[test]
    fn reject_insert() {
        let mut cache = Cache::new(2).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert!(cache.insert(2, 2).is_ok());
        let k = cache.key_hash(&1);
        cache.admit.get_mut().unwrap().increment(&k);
        if let Err(_) = cache.insert(4, 4) {
            assert!(cache.contains(&1));
            assert!(!cache.contains(&2), "Victim should be value 2");
        } else {
            assert!(false, "Item should be reject because of low estimate");
        }
    }

    #[test]
    fn contains() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert!(cache.contains(&1));
        assert!(!cache.contains(&2));
    }

    #[test]
    fn remove() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.insert(1, 3).is_ok());
        let removed = cache.remove(&1);
        assert!(removed.is_some());
        if let Some(removed) = removed {
            assert_eq!(removed, 3);
        }
        assert!(cache.remove(&2).is_none());
        assert!(!cache.contains(&1));
        assert!(!cache.contains(&2));
    }

    #[test]
    fn room_left() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert_eq!(cache.room_left(), 9);
    }
    #[test]
    fn capacity() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert_eq!(cache.capacity(), 10);
    }

    #[test]
    fn clear() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.insert(1, 1).is_ok());
        assert!(cache.insert(2, 2).is_ok());
        assert!(cache.insert(3, 2).is_ok());
        cache.clear();
        assert!(!cache.contains(&1));
        assert!(!cache.contains(&2));
        assert!(!cache.contains(&3));
        assert_eq!(cache.room_left(), 10);
    }

    #[test]
    fn is_empty() {
        let mut cache = Cache::new(10).with_metrics();
        assert!(cache.is_empty());
        assert!(cache.insert(1, 1).is_ok());
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }
}
