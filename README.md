# An implementation of TinyLFU cache

[![Build Status](https://travis-ci.org/selmeci/cascara.svg?branch=master)](https://travis-ci.org/selmeci/cascara)
[![Latest Version](https://img.shields.io/crates/v/cascara.svg)](https://crates.io/crates/cascara)
[![Docs](https://docs.rs/cascara/badge.svg)](https://docs.rs/cascara)
[![Coverage Status](https://coveralls.io/repos/github/selmeci/cascara/badge.svg?branch=master)](https://coveralls.io/github/selmeci/cascara?branch=master)

[TinyLFU](https://arxiv.org/abs/1512.00727) proposes to use a frequency based cache admission policy in order to boost the effectiveness of caches subject to skewed access distributions. Given a newly accessed item and an eviction candidate from the cache, TinyLFU scheme decides, based on the recent access history, whether it is worth admitting the new item into the cache at the expense of the eviction candidate. TinyLFU maintains an approximate representation of the access frequency of a large sample of recently accessed items. TinyLFU is very compact and light-weight as it builds upon Bloom filter theory.

This repository implements TinyLFU with help of [probabilistic_collections](https://crates.io/crates/probabilistic-collections) crate.

Cache provides: `insert`, `insert_with_ttl`, `get`, `get_mut`, `remove`, `contains`, `is_empty` operations.

## Example

```rust
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
    let mut cache = Cache::with_on_evict(10, 20, Evict::default())
        .with_metrics();
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
    println!(
        "\nCache metrics. {:?}",
        cache.metrics().expect("Cache should have metrics")
    )
}


```
