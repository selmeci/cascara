#![feature(test)]

extern crate cascara;
extern crate test;

use test::Bencher;

use cascara::Cache;

#[bench]
fn insert(b: &mut Bencher) {
    b.iter(|| {
        let mut cache = Cache::new(100, 200);
        for i in 0..1000 {
            cache.insert(i, i);
        }
    })
}

#[bench]
fn get(b: &mut Bencher) {
    let mut cache = Cache::new(100, 200);
    for i in 0..1000 {
        cache.insert(i, i);
    }
    b.iter(|| {
        for i in 0..1000 {
            cache.get(&i);
        }
    })
}
