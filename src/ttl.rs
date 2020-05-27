use std::collections::{BTreeMap, HashSet};
use std::ops::Add;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

///
/// Calculate bucket id as duration in seconds from UNIX epoch
///
fn storage_bucket(expiration_time: &SystemTime) -> u64 {
    expiration_time
        .duration_since(UNIX_EPOCH)
        .expect("Unix Epoch")
        .as_secs()
}

///
/// Manage bucket of expiration times for items.
/// Expiration is defined in seconds.
///
pub trait Expiration {
    ///
    /// Insert expiration for given key.
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    /// - `expiration`: duration in seconds of item from now
    ///
    /// # Return
    ///
    /// If expiration in seconds == 0, than return None.
    /// Else return system time when item will be expired.
    ///
    fn insert(&mut self, k: u64, expiration: Duration) -> Option<SystemTime>;

    ///
    /// Update expiration time for given key.
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    /// - `expiration_time`: actual expiration time for item.
    /// - `new_expiration`: duration in seconds of item from now
    ///
    /// # Return
    ///
    /// If expiration in seconds == 0, than return None.
    /// Else return system time when item will be expired.
    ///
    fn update(
        &mut self,
        k: u64,
        expiration_time: &SystemTime,
        new_expiration: Duration,
    ) -> Option<SystemTime>;

    ///
    /// Remove item expired in given time from buckets.
    ///
    /// # Arguments
    ///
    /// - `k`: item identification
    /// - `expiration_time`: actual expiration time for item.
    ///
    /// # Return
    ///
    /// True if item was removed
    ///
    fn remove(&mut self, k: &u64, expiration_time: &SystemTime) -> bool;

    ///
    /// Remove all item from buckets which are expired at the moment.
    ///
    /// # Arguments
    ///
    /// - `now`: All items expired before this time are marked
    ///
    /// # Return
    ///
    /// HashSet with removed items.
    ///
    fn cleanup(&mut self, now: &SystemTime) -> HashSet<u64>;

    ///
    /// Remove all items
    ///
    fn clear(&mut self);

    ///
    /// Check if all expiration buckets are empty
    ///
    fn is_empty(&self) -> bool;
}

///
/// ExpirationMap holds items in BTreeMap, where key is expiration time in secs from UNIX epoch for given bucket of items.
///
#[derive(Clone, Debug)]
pub struct ExpirationMap {
    buckets: BTreeMap<u64, HashSet<u64>>,
}

impl ExpirationMap {
    ///
    /// Create new expiration map
    ///
    pub fn new() -> Self {
        Self {
            buckets: BTreeMap::new(),
        }
    }
}

impl Expiration for ExpirationMap {
    fn insert(&mut self, k: u64, expiration: Duration) -> Option<SystemTime> {
        if expiration.as_secs() == 0 {
            return None;
        }
        let expiration_time = SystemTime::now().add(expiration);
        let bucket_num = storage_bucket(&expiration_time);
        if let Some(bucket) = self.buckets.get_mut(&bucket_num) {
            bucket.insert(k);
        } else {
            let mut bucket = HashSet::new();
            bucket.insert(k);
            self.buckets.insert(bucket_num, bucket);
        }
        Some(expiration_time)
    }

    fn update(
        &mut self,
        k: u64,
        expiration_time: &SystemTime,
        new_expiration: Duration,
    ) -> Option<SystemTime> {
        self.remove(&k, expiration_time);
        self.insert(k, new_expiration)
    }

    fn remove(&mut self, k: &u64, expiration_time: &SystemTime) -> bool {
        let old_bucket_num = storage_bucket(expiration_time);
        if let Some(bucket) = self.buckets.get_mut(&old_bucket_num) {
            bucket.remove(k)
        } else {
            false
        }
    }

    fn cleanup(&mut self, now: &SystemTime) -> HashSet<u64> {
        let now = storage_bucket(now) + 1;
        let mut result = HashSet::new();
        let mut buckets = Vec::new();
        for (id, _) in self.buckets.range(..now) {
            buckets.push(id.clone())
        }
        for bucket in buckets {
            for item in self.buckets.remove(&bucket).unwrap() {
                result.insert(item);
            }
        }
        result
    }

    fn clear(&mut self) {
        self.buckets.clear();
    }

    fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::ttl::{Expiration, ExpirationMap};
    use std::ops::Add;
    use std::time::{Duration, SystemTime};

    #[test]
    fn insert_and_remove() {
        let mut expiration_map = ExpirationMap::new();
        let expiration = expiration_map.insert(0, Duration::from_secs(5)).unwrap();
        assert!(!expiration_map.remove(&1, &expiration));
        assert!(expiration_map.remove(&0, &expiration));
    }

    #[test]
    fn cleanup() {
        let mut expiration_map = ExpirationMap::new();
        expiration_map.insert(0, Duration::from_secs(1));
        expiration_map.insert(1, Duration::from_secs(1));
        expiration_map.insert(2, Duration::from_secs(3));
        std::thread::sleep(Duration::from_secs(2));
        let mut removed = expiration_map.cleanup(&SystemTime::now());
        assert!(removed.remove(&0));
        assert!(removed.remove(&1));
        assert!(removed.is_empty());
        std::thread::sleep(Duration::from_secs(2));
        let mut removed = expiration_map.cleanup(&SystemTime::now());
        assert!(removed.remove(&2));
        assert!(removed.is_empty());
        assert!(expiration_map.is_empty());
    }

    #[test]
    fn cleanup_unordered() {
        let mut expiration_map = ExpirationMap::new();
        expiration_map.insert(0, Duration::from_secs(10));
        expiration_map.insert(1, Duration::from_secs(1));
        expiration_map.insert(2, Duration::from_secs(3));
        std::thread::sleep(Duration::from_secs(2));
        let mut removed = expiration_map.cleanup(&SystemTime::now());
        assert!(removed.remove(&1));
        assert!(removed.is_empty());
        std::thread::sleep(Duration::from_secs(2));
        let mut removed = expiration_map.cleanup(&SystemTime::now());
        assert!(removed.remove(&2));
        assert!(removed.is_empty());
        assert!(!expiration_map.is_empty());
    }

    #[test]
    fn clear() {
        let mut expiration_map = ExpirationMap::new();
        expiration_map.insert(0, Duration::from_secs(1));
        expiration_map.insert(1, Duration::from_secs(1));
        expiration_map.insert(2, Duration::from_secs(3));
        expiration_map.clear();
        assert!(expiration_map.is_empty());
    }

    #[test]
    fn do_not_register_zero_duration() {
        let mut expiration_map = ExpirationMap::new();
        assert!(expiration_map.insert(0, Duration::from_secs(0)).is_none());
    }

    #[test]
    fn remove() {
        let mut expiration_map = ExpirationMap::new();
        let expiration_time = expiration_map.insert(0, Duration::from_secs(1));
        assert!(expiration_time.is_some());
        if let Some(expiration_time) = expiration_time {
            assert!(expiration_map.remove(&0, &expiration_time));
            assert!(!expiration_map.remove(&1, &expiration_time));
        }
    }

    #[test]
    fn remove_not_existing() {
        let mut expiration_map = ExpirationMap::new();
        let expiration_time = SystemTime::now().add(Duration::from_secs(10));
        assert!(!expiration_map.remove(&1, &expiration_time));
    }

    #[test]
    fn update() {
        let mut expiration_map = ExpirationMap::new();
        let expiration_time = expiration_map.insert(0, Duration::from_secs(1));
        assert!(expiration_time.is_some());
        if let Some(expiration_time) = expiration_time {
            let new_expiration_time =
                expiration_map.update(0, &expiration_time, Duration::from_secs(1));
            assert!(new_expiration_time.is_some());
            if let Some(new_expiration_time) = new_expiration_time {
                assert!(expiration_map.remove(&0, &new_expiration_time));
            }
        }
    }
}
