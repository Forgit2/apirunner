use super::*;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Performance benchmarks for execution strategies
pub struct ExecutionBenchmarks;

impl ExecutionBenchmarks {
    /// Benchmark HTTP request execution performance
    pub async fn benchmark_http_requests(test_count: usize) -> Duration {
        let test_server = get_test_server().await;
        
        let start_time = Instant::now();
        
        // Simulate HTTP requests for benchmarking
        let client = reqwest::Client::new();
        let mut handles = Vec::new();
        
        for _i in 0..test_count {
            let client = client.clone();
            let url = format!("{}/api/v1/users", test_server.base_url);
            let handle = tokio::spawn(async move {
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
    
    /// Benchmark concurrent HTTP request execution
    pub async fn benchmark_concurrent_requests(test_count: usize, concurrency: usize) -> Duration {
        let test_server = get_test_server().await;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
        
        let start_time = Instant::now();
        
        let client = reqwest::Client::new();
        let mut handles = Vec::new();
        
        for _i in 0..test_count {
            let client = client.clone();
            let url = format!("{}/api/v1/users", test_server.base_url);
            let semaphore = semaphore.clone();
            
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
        
        let test_server = get_test_server().await;
        let client = reqwest::Client::new();
        
        let start_time = Instant::now();
        
        // Create and execute many HTTP requests to measure memory usage
        let mut handles = Vec::new();
        for _i in 0..test_count {
            let client = client.clone();
            let url = format!("{}/api/v1/users", test_server.base_url);
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
    
    /// Benchmark execution with simulated network delays
    pub async fn benchmark_with_network_delays(
        test_count: usize,
        network_delay_ms: u64,
    ) -> Duration {
        let test_server = get_test_server().await;
        let client = reqwest::Client::new();
        
        let start_time = Instant::now();
        
        for _i in 0..test_count {
            // Simulate network delay
            if network_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(network_delay_ms)).await;
            }
            
            let url = format!("{}/api/v1/users", test_server.base_url);
            let _response = client.get(&url).send().await;
        }
        
        start_time.elapsed()
    }
}



/// Criterion benchmark functions
fn benchmark_http_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("http_requests");
    for test_count in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("test_count", test_count),
            test_count,
            |b, &test_count| {
                b.iter(|| {
                    rt.block_on(async {
                        ExecutionBenchmarks::benchmark_http_requests(test_count).await
                    })
                });
            },
        );
    }
    group.finish();
}

fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_requests");
    for (test_count, concurrency) in [(50, 5), (100, 10), (200, 20), (500, 50)].iter() {
        group.bench_with_input(
            BenchmarkId::new("test_count_concurrency", format!("{}_{}", test_count, concurrency)),
            &(*test_count, *concurrency),
            |b, &(test_count, concurrency)| {
                b.iter(|| {
                    rt.block_on(async {
                        ExecutionBenchmarks::benchmark_concurrent_requests(test_count, concurrency).await
                    })
                });
            },
        );
    }
    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_usage");
    for test_count in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("test_count", test_count),
            test_count,
            |b, &test_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let (duration, _memory_used) = ExecutionBenchmarks::benchmark_memory_usage(test_count).await;
                        duration
                    })
                });
            },
        );
    }
    group.finish();
}

fn benchmark_network_delays(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("network_delays");
    let scenarios = [
        (50, 10),   // 50 tests, 10ms delay
        (100, 50),  // 100 tests, 50ms delay
        (200, 100), // 200 tests, 100ms delay
    ];
    
    for (test_count, delay_ms) in scenarios.iter() {
        group.bench_with_input(
            BenchmarkId::new(
                "scenario",
                format!("tests_{}_delay_{}ms", test_count, delay_ms)
            ),
            &(*test_count, *delay_ms),
            |b, &(test_count, delay_ms)| {
                b.iter(|| {
                    rt.block_on(async {
                        ExecutionBenchmarks::benchmark_with_network_delays(test_count, delay_ms).await
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
    benchmark_memory_usage,
    benchmark_network_delays
);
criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_http_requests_benchmark() {
        let duration = ExecutionBenchmarks::benchmark_http_requests(10).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("HTTP requests benchmark of 10 tests took: {:?}", duration);
    }
    
    #[tokio::test]
    async fn test_concurrent_requests_benchmark() {
        let duration = ExecutionBenchmarks::benchmark_concurrent_requests(20, 5).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("Concurrent requests benchmark of 20 tests with concurrency 5 took: {:?}", duration);
    }
    
    #[tokio::test]
    async fn test_memory_usage_benchmark() {
        let (duration, memory_used) = ExecutionBenchmarks::benchmark_memory_usage(50).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("Memory benchmark of 50 tests took: {:?}, used {} KB", 
                duration, memory_used / 1024);
    }
    
    #[tokio::test]
    async fn test_network_delays_benchmark() {
        let duration = ExecutionBenchmarks::benchmark_with_network_delays(10, 20).await;
        assert!(duration.as_millis() > 0, "Benchmark should take measurable time");
        println!("Network delays benchmark took: {:?}", duration);
    }
}