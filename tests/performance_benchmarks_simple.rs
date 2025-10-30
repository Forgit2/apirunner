use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Simple performance benchmarks for HTTP requests
pub struct SimpleBenchmarks;

impl SimpleBenchmarks {
    /// Benchmark basic HTTP request performance
    pub async fn benchmark_http_requests(test_count: usize) -> Duration {
        let client = reqwest::Client::new();
        let start_time = Instant::now();
        
        // Use httpbin.org for testing (publicly available HTTP testing service)
        let url = "https://httpbin.org/get";
        
        for _i in 0..test_count {
            let _response = client.get(url).send().await;
        }
        
        start_time.elapsed()
    }
    
    /// Benchmark concurrent HTTP requests
    pub async fn benchmark_concurrent_requests(test_count: usize, concurrency: usize) -> Duration {
        let client = reqwest::Client::new();
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
        let start_time = Instant::now();
        
        let url = "https://httpbin.org/get";
        let mut handles = Vec::new();
        
        for _i in 0..test_count {
            let client = client.clone();
            let semaphore = semaphore.clone();
            let url = url.to_string();
            
            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let _response = client.get(&url).send().await;
            });
            handles.push(handle);
        }
        
        // Wait for all requests to complete
        for handle in handles {
            let _ = handle.await;
        }
        
        start_time.elapsed()
    }
    
    /// Benchmark memory usage during execution
    pub async fn benchmark_memory_usage(test_count: usize) -> (Duration, u64) {
        use sysinfo::{System, SystemExt, ProcessExt};
        
        let mut system = System::new_all();
        system.refresh_all();
        
        let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
        let initial_memory = system.process(pid)
            .map(|p| p.memory())
            .unwrap_or(0);
        
        let client = reqwest::Client::new();
        let start_time = Instant::now();
        
        let url = "https://httpbin.org/get";
        
        // Create and execute many HTTP requests to measure memory usage
        let mut handles = Vec::new();
        for _i in 0..test_count {
            let client = client.clone();
            let url = url.to_string();
            let handle = tokio::spawn(async move {
                let _response = client.get(&url).send().await;
            });
            handles.push(handle);
        }
        
        // Wait for all requests to complete
        for handle in handles {
            let _ = handle.await;
        }
        
        let duration = start_time.elapsed();
        
        system.refresh_all();
        let final_memory = system.process(pid)
            .map(|p| p.memory())
            .unwrap_or(0);
        
        let memory_used = final_memory.saturating_sub(initial_memory);
        (duration, memory_used)
    }
}

/// Criterion benchmark functions
fn benchmark_http_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("simple_http_requests");
    for test_count in [5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("test_count", test_count),
            test_count,
            |b, &test_count| {
                b.iter(|| {
                    rt.block_on(async {
                        SimpleBenchmarks::benchmark_http_requests(test_count).await
                    })
                });
            },
        );
    }
    group.finish();
}

fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("simple_concurrent_requests");
    for (test_count, concurrency) in [(10, 2), (20, 5)].iter() {
        group.bench_with_input(
            BenchmarkId::new("test_count_concurrency", format!("{}_{}", test_count, concurrency)),
            &(*test_count, *concurrency),
            |b, &(test_count, concurrency)| {
                b.iter(|| {
                    rt.block_on(async {
                        SimpleBenchmarks::benchmark_concurrent_requests(test_count, concurrency).await
                    })
                });
            },
        );
    }
    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("simple_memory_usage");
    for test_count in [10, 25, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("test_count", test_count),
            test_count,
            |b, &test_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let (duration, _memory_used) = SimpleBenchmarks::benchmark_memory_usage(test_count).await;
                        duration
                    })
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_http_requests,
    benchmark_concurrent_requests,
    benchmark_memory_usage
);
criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_http_requests_benchmark() {
        let duration = SimpleBenchmarks::benchmark_http_requests(3).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("HTTP requests benchmark of 3 tests took: {:?}", duration);
    }
    
    #[tokio::test]
    async fn test_concurrent_requests_benchmark() {
        let duration = SimpleBenchmarks::benchmark_concurrent_requests(5, 2).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("Concurrent requests benchmark of 5 tests with concurrency 2 took: {:?}", duration);
    }
    
    #[tokio::test]
    async fn test_memory_usage_benchmark() {
        let (duration, memory_used) = SimpleBenchmarks::benchmark_memory_usage(10).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("Memory benchmark of 10 tests took: {:?}, used {} KB", 
                duration, memory_used / 1024);
    }
}