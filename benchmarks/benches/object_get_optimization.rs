use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sonic_rs::{Object, Value, from_str};

fn create_small_object(size: usize) -> Object {
    let mut obj = Object::new();
    for i in 0..size {
        obj.insert(&format!("key{:02}", i), Value::from(i));
    }
    obj
}

fn create_medium_object(size: usize) -> Object {
    let mut obj = Object::new();
    for i in 0..size {
        // Use longer keys to benefit from SIMD optimization
        obj.insert(&format!("medium_key_name_{:03}", i), Value::from(i));
    }
    obj
}

fn create_large_object(size: usize) -> Object {
    let mut obj = Object::new();
    for i in 0..size {
        obj.insert(&format!("large_object_key_{:04}", i), Value::from(i * 10));
    }
    obj
}

fn bench_small_objects(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("small_objects_1_to_7_keys");
    
    for size in [1, 3, 5, 7] {
        let obj = create_small_object(size);
        let test_key = format!("key{:02}", size / 2); // middle key
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("optimized_get", size),
            &(obj, test_key),
            |b, (obj, key)| {
                b.iter(|| obj.get(key))
            }
        );
    }
    group.finish();
}

fn bench_medium_objects(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("medium_objects_8_to_31_keys");
    
    for size in [8, 15, 20, 31] {
        let obj = create_medium_object(size);
        let test_key = format!("medium_key_name_{:03}", size / 2); // middle key
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("simd_optimized_get", size),
            &(obj, test_key),
            |b, (obj, key)| {
                b.iter(|| obj.get(key))
            }
        );
    }
    group.finish();
}

fn bench_large_objects(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("large_objects_32_plus_keys");
    
    for size in [32, 50, 100, 200] {
        let obj = create_large_object(size);
        let test_key = format!("large_object_key_{:04}", size / 2); // middle key
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("hash_index_get", size),
            &(obj, test_key),
            |b, (obj, key)| {
                b.iter(|| obj.get(key))
            }
        );
    }
    group.finish();
}

fn bench_different_key_positions(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("key_position_impact");
    
    let obj = create_large_object(100);
    
    // Test first, middle, and last key positions
    let positions = [
        ("first", "large_object_key_0000"),
        ("middle", "large_object_key_0050"),
        ("last", "large_object_key_0099"),
    ];
    
    for (pos_name, key) in positions {
        group.bench_with_input(
            BenchmarkId::new("get_by_position", pos_name),
            &key,
            |b, key| {
                b.iter(|| obj.get(key))
            }
        );
    }
    group.finish();
}

fn bench_cache_behavior(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("cache_behavior");
    
    let obj = create_large_object(100);
    let test_key = "large_object_key_0050";
    
    group.bench_function("repeated_lookups", |b| {
        b.iter(|| {
            // Perform multiple lookups to test cache effectiveness
            for _ in 0..10 {
                obj.get(&test_key);
            }
        })
    });
    
    group.finish();
}

fn bench_key_length_impact(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("key_length_impact");
    
    // Test with different key lengths to evaluate SIMD effectiveness
    let test_cases = [
        ("short", 4, "k"),
        ("medium", 16, "medium_length_key"),
        ("long", 32, "very_long_key_name_that_should_benefit_from_simd"),
    ];
    
    for (name, obj_size, key_prefix) in test_cases {
        let mut obj = Object::new();
        for i in 0..obj_size {
            let key = format!("{}_{:03}", key_prefix, i);
            obj.insert(&key, Value::from(i));
        }
        
        let test_key = format!("{}_{:03}", key_prefix, obj_size / 2);
        
        group.bench_with_input(
            BenchmarkId::new("get_by_key_length", name),
            &(obj, test_key),
            |b, (obj, key)| {
                b.iter(|| obj.get(key))
            }
        );
    }
    group.finish();
}

fn bench_real_world_patterns(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);

    let mut group = c.benchmark_group("real_world_patterns");
    
    // Simulate common JSON patterns
    let json_configs = [
        ("api_response", r#"{"status": "success", "data": {"id": 123, "name": "John", "email": "john@example.com"}, "timestamp": "2024-01-01T00:00:00Z", "version": "1.0"}"#),
        ("user_profile", r#"{"userId": 12345, "username": "johndoe", "firstName": "John", "lastName": "Doe", "email": "john.doe@example.com", "birthDate": "1990-01-01", "isActive": true, "roles": ["user", "admin"], "preferences": {"theme": "dark", "language": "en"}, "lastLogin": "2024-01-01T12:00:00Z"}"#),
    ];
    
    for (name, json) in json_configs {
        let obj: Object = from_str(json).unwrap();
        
        // Test common access patterns
        let test_keys = match name {
            "api_response" => vec!["status", "data", "timestamp"],
            "user_profile" => vec!["userId", "email", "isActive", "preferences"],
            _ => vec!["status"],
        };
        
        for key in test_keys {
            group.bench_with_input(
                BenchmarkId::new(name, key),
                &(obj.clone(), key),
                |b, (obj, key)| {
                    b.iter(|| obj.get(key))
                }
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_small_objects,
    bench_medium_objects,
    bench_large_objects,
    bench_different_key_positions,
    bench_cache_behavior,
    bench_key_length_impact,
    bench_real_world_patterns
);
criterion_main!(benches);