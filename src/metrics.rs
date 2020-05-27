use std::fmt::{self, Debug, Formatter};

const METRICS: usize = 6;

///
/// Possible metric types
///
#[derive(Debug, Clone)]
pub enum MetricType {
    Hit,
    Miss,
    KeyInsert,
    KeyUpdate,
    KeyEvict,
}

impl MetricType {
    fn metric_idx(&self) -> usize {
        match self {
            MetricType::Hit => 0,
            MetricType::Miss => 1,
            MetricType::KeyInsert => 2,
            MetricType::KeyUpdate => 3,
            MetricType::KeyEvict => 4,
        }
    }
}

///
/// Collector of possible metrics types in cache
///
#[derive(Clone)]
pub struct Metrics {
    all: [[usize; 256]; METRICS],
}

impl Metrics {
    ///
    /// Create new metrics
    ///
    pub fn new() -> Self {
        Self {
            all: [[0; 256]; METRICS],
        }
    }

    ///
    /// Insert delta for given metric type
    ///
    pub fn insert(&mut self, metric: MetricType, k: &u64, delta: usize) {
        let idx = (k % 25) * 10;
        let vals = &mut self.all[metric.metric_idx()];
        vals[idx as usize] += delta;
    }

    ///
    /// Get collected data about metric type
    ///
    pub fn get(&self, metric: MetricType) -> usize {
        let vals = &self.all[metric.metric_idx()];
        vals.iter().sum()
    }

    ///
    /// Collected hits metrics
    ///
    pub fn hits(&self) -> usize {
        self.get(MetricType::Hit)
    }

    ///
    /// Collected misses metrics
    ///
    pub fn misses(&self) -> usize {
        self.get(MetricType::Miss)
    }

    ///
    /// Collected keys inserted metrics
    ///
    pub fn keys_inserted(&self) -> usize {
        self.get(MetricType::KeyInsert)
    }

    ///
    /// Collected keys updated metrics
    ///
    pub fn keys_updated(&self) -> usize {
        self.get(MetricType::KeyUpdate)
    }

    ///
    /// Collected keys evicted metrics
    ///
    pub fn keys_evicted(&self) -> usize {
        self.get(MetricType::KeyEvict)
    }

    ///
    /// Collected hits/misses ratio metrics
    ///
    pub fn ratio(&self) -> f64 {
        let hits = self.hits();
        let misses = self.misses();
        if hits == 0 && misses == 0 {
            return 0.0;
        }
        hits as f64 / (hits + misses) as f64
    }

    ///
    /// Clear all collected metrics data for every category
    ///
    pub fn clear(&mut self) {
        self.all = [[0; 256]; METRICS];
    }
}

impl Debug for Metrics {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metrics")
            .field("hits", &self.hits())
            .field("misses", &self.misses())
            .field("keys_inserted", &self.keys_inserted())
            .field("keys_updated", &self.keys_updated())
            .field("keys_evicted", &self.keys_evicted())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::{MetricType, Metrics};

    #[test]
    fn metric_types() {
        let mut metrics = Metrics::new();
        for i in 0..10 {
            metrics.insert(MetricType::Hit, &i, 1);
            metrics.insert(MetricType::Miss, &i, 1);
            metrics.insert(MetricType::KeyInsert, &i, 1);
            metrics.insert(MetricType::KeyUpdate, &i, 1);
            metrics.insert(MetricType::KeyEvict, &i, 1);
        }
        assert_eq!(metrics.hits(), 10);
        assert_eq!(metrics.misses(), 10);
        assert_eq!(metrics.keys_inserted(), 10);
        assert_eq!(metrics.keys_updated(), 10);
        assert_eq!(metrics.keys_evicted(), 10);
    }

    #[test]
    fn hits() {
        let mut metrics = Metrics::new();
        for i in 0..10 {
            metrics.insert(MetricType::Hit, &i, 1);
        }
        assert_eq!(metrics.hits(), 10);
    }

    #[test]
    fn misses() {
        let mut metrics = Metrics::new();
        for i in 0..10 {
            metrics.insert(MetricType::Miss, &i, 1);
        }
        assert_eq!(metrics.misses(), 10);
    }

    #[test]
    fn keys_inserted() {
        let mut metrics = Metrics::new();
        for i in 0..10 {
            metrics.insert(MetricType::KeyInsert, &i, 1);
        }
        assert_eq!(metrics.keys_inserted(), 10);
    }

    #[test]
    fn keys_updated() {
        let mut metrics = Metrics::new();
        for i in 0..10 {
            metrics.insert(MetricType::KeyUpdate, &i, 1);
        }
        assert_eq!(metrics.keys_updated(), 10);
    }

    #[test]
    fn keys_evicted() {
        let mut metrics = Metrics::new();
        for i in 0..10 {
            metrics.insert(MetricType::KeyEvict, &i, 1);
        }
        assert_eq!(metrics.keys_evicted(), 10);
    }

    #[test]
    fn get() {
        let mut metrics = Metrics::new();
        metrics.insert(MetricType::Hit, &1, 1);
        metrics.insert(MetricType::Hit, &2, 2);
        metrics.insert(MetricType::Hit, &3, 3);
        assert_eq!(metrics.get(MetricType::Hit), 6);
    }

    #[test]
    fn ratio() {
        let mut metrics = Metrics::new();
        assert_eq!(0.0, metrics.ratio());
        metrics.insert(MetricType::Hit, &1, 1);
        metrics.insert(MetricType::Hit, &2, 2);
        metrics.insert(MetricType::Miss, &1, 1);
        metrics.insert(MetricType::Miss, &2, 2);
        assert_eq!(metrics.ratio(), 0.5)
    }

    #[test]
    fn clear() {
        let mut metrics = Metrics::new();
        metrics.insert(MetricType::Hit, &1, 1);
        metrics.insert(MetricType::Hit, &2, 2);
        metrics.insert(MetricType::Miss, &1, 1);
        metrics.insert(MetricType::Miss, &2, 2);
        assert_eq!(metrics.ratio(), 0.5);
        metrics.clear();
        assert_eq!(metrics.ratio(), 0.0);
        assert_eq!(metrics.hits(), 0);
        assert_eq!(metrics.misses(), 0);
    }

    #[test]
    fn debug() {
        let mut metrics = Metrics::new();
        metrics.insert(MetricType::Hit, &1, 1);
        metrics.insert(MetricType::Miss, &2, 2);
        metrics.insert(MetricType::KeyEvict, &1, 1);
        metrics.insert(MetricType::KeyUpdate, &2, 2);
        let dbg = format!("{:?}", metrics);
        assert_eq!(
            dbg,
            "Metrics { hits: 1, misses: 2, keys_inserted: 0, keys_updated: 2, keys_evicted: 1 }"
                .to_string()
        );
    }
}
