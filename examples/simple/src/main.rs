extern crate cascara;

use cascara::{Cache, OnEvict};
use std::time::Duration;

#[derive(Default, Debug)]
struct Evict {}

impl OnEvict<usize, usize> for Evict {
    fn evict(&self, k: &usize, v: &usize) {
        println!("Evict item.  k={}, v={}", k, v);
    }
}

fn main() {
    //create cache with activated metrics collecting(mis, hit, insert, update, evict)
    let mut cache = Cache::with_on_evict(10, Evict::default()).with_metrics();
    assert!(cache.is_empty());
    assert_eq!(cache.get(&1), None);
    cache.insert(1, 1).expect("Item is not inserted");
    assert_eq!(cache.get(&1), Some(&1));
    let previous = cache.insert(1, 2).expect("Item is not updated");
    assert_eq!(previous, Some(1));
    assert_eq!(cache.get(&1), Some(&2));
    cache
        .insert_with_ttl(2, 2, Duration::from_secs(1))
        .expect("Item is not inserted");
    assert!(cache.contains(&2));
    std::thread::sleep(Duration::from_secs(2));
    assert!(!cache.contains(&2));
    {
        let v = cache.get_mut(&1).unwrap();
        *v = 3;
    }
    assert_eq!(cache.get(&1), Some(&3));
    for i in 0..25 {
        match cache.insert(i, i) {
            Ok(_) => println!("Item is inserted. i: {}", i),
            Err(_) => println!("Item is rejected. i: {}", i),
        }
    }
    for (k, v) in cache.iter() {
        println!("Item: k: {}, v: {}", k, v);
    }
    println!(
        "\nCache metrics. {:?}",
        cache.metrics().expect("Cache should have metrics")
    )
}
