use probabilistic_collections::count_min_sketch::{CountMinSketch, CountMinStrategy};
use probabilistic_collections::cuckoo::CuckooFilter;
use std::cmp;
use std::collections::HashSet;

///
/// Max window size before sketcher is reset
///
pub const MAX_WINDOW_SIZE: usize = 10000;

pub trait TinyLFU {
    ///
    /// Returns the estimated number of times `k` is in the count-min sketch + if it is identified in cuckoo filter.
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    ///
    fn estimate(&self, k: &u64) -> i64;

    ///
    /// Add item into filter if missing. If item is already in filter increase it in count-min sketcher.
    ///
    /// # Arguments
    ///
    /// - `k`: item key
    ///
    fn increment(&mut self, k: &u64);

    ///
    /// Reset counts for every item in window ot half
    ///
    fn reset(&mut self);

    ///
    /// Clear filter and count-min sketcher.
    ///
    fn clear(&mut self);
}

///
/// TinyLFU implementation with application CountMinSketcher and CuckooFilter.
///
pub struct TinyLFUCache {
    sketcher: CountMinSketch<CountMinStrategy, u64>,
    filter: CuckooFilter<u64>,
    increments: usize,
    window_size: usize,
    actual_window: HashSet<u64>,
    previous_window: HashSet<u64>,
}

impl TinyLFUCache {
    ///
    /// Create new instance of TinyLFU with defined windows size.
    ///
    /// # Arguments
    ///
    /// - `window_size`: How many increments can be done, before tinyLfu is reset.
    ///
    /// # Panic
    ///
    /// When `windows_size` == 0
    ///
    pub fn new(window_size: usize) -> Self {
        assert_ne!(window_size, 0);
        let window_size = cmp::min(window_size, MAX_WINDOW_SIZE);
        Self {
            sketcher: CountMinSketch::from_error(0.1, 0.05),
            filter: CuckooFilter::from_entries_per_index(window_size, 0.01, 8),
            window_size,
            increments: 0,
            actual_window: HashSet::new(),
            previous_window: HashSet::new(),
        }
    }

    fn reset_sketcher(&mut self) {
        for item in self.previous_window.drain() {
            let hits = self.sketcher.count(&item);
            self.sketcher.insert(&item, -hits);
        }
        let mut tmp = HashSet::new();
        for item in self.actual_window.drain() {
            let hits = self.sketcher.count(&item);
            self.sketcher.insert(&item, -((hits / 2) + (hits % 2)));
            tmp.insert(item);
        }
        self.previous_window = tmp;
    }
}

impl TinyLFU for TinyLFUCache {
    fn estimate(&self, k: &u64) -> i64 {
        let mut hits = self.sketcher.count(k);
        if self.filter.contains(k) {
            hits += 1;
        }
        hits
    }

    fn increment(&mut self, k: &u64) {
        if self.increments >= self.window_size {
            self.reset()
        }
        if !self.filter.contains(k) {
            self.filter.insert(k);
        } else {
            self.sketcher.insert(k, 1);
            self.previous_window.remove(k);
            self.actual_window.insert(k.clone());
        }
        self.increments += 1;
    }

    fn reset(&mut self) {
        self.reset_sketcher();
        self.filter.clear();
        self.increments = 0;
    }

    fn clear(&mut self) {
        self.sketcher.clear();
        self.filter.clear();
        self.increments = 0;
    }
}

#[cfg(test)]
mod tests {
    use crate::tiny_lfu::{TinyLFU, TinyLFUCache};

    #[test]
    fn increment() {
        let mut tiny = TinyLFUCache::new(4);
        tiny.increment(&1);
        tiny.increment(&1);
        tiny.increment(&1);
        tiny.increment(&1);
        assert!(tiny.filter.contains(&1));
        assert_eq!(tiny.sketcher.count(&1), 3);
        tiny.increment(&1);
        assert!(tiny.filter.contains(&1));
        assert_eq!(tiny.sketcher.count(&1), 1);
        tiny.increment(&2);
        tiny.increment(&2);
        tiny.increment(&2);
        tiny.increment(&2);
        assert!(!tiny.filter.contains(&1));
        assert_eq!(tiny.sketcher.count(&1), 0);
    }

    #[test]
    fn estimate() {
        let mut tiny = TinyLFUCache::new(8);
        tiny.increment(&1);
        tiny.increment(&1);
        tiny.increment(&1);
        assert_eq!(tiny.estimate(&1), 3);
        assert_eq!(tiny.estimate(&2), 0);
    }

    #[test]
    fn clear() {
        let mut tiny = TinyLFUCache::new(16);
        tiny.increment(&1);
        tiny.increment(&2);
        tiny.increment(&2);
        tiny.increment(&2);
        tiny.clear();
        assert_eq!(tiny.increments, 0);
        assert_eq!(tiny.estimate(&2), 0);
    }
}
