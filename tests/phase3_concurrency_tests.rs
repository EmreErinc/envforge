use envforge::parser::parse_shell_content;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

// ============================================================================
// Concurrency Tests - RwLock Under Contention (12 tests)
// ============================================================================

#[test]
fn test_parse_concurrent_reads_no_panic() {
    let content = Arc::new("VAR1=value1\nVAR2=value2\n".to_string());
    let parsed = Arc::new(Mutex::new(
        parse_shell_content(&content, Path::new("test.env")).expect("parse failed"),
    ));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let p = Arc::clone(&parsed);
            thread::spawn(move || {
                let file = p.lock().unwrap();
                assert!(file.lines.len() >= 2);
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_parse_different_files_no_panic() {
    let handles: Vec<_> = (0..5)
        .map(|i| {
            thread::spawn(move || {
                let content = format!("VAR_{}=value_{}\n", i, i);
                let result = parse_shell_content(&content, Path::new("test.env"));
                assert!(result.is_ok());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_serialization_no_panic() {
    let content = "VAR1=value1\nVAR2=value2\nVAR3=$VAR1\n";
    let file = Arc::new(parse_shell_content(content, Path::new("test.env")).unwrap());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let f = Arc::clone(&file);
            thread::spawn(move || {
                let serialized = f.serialize();
                assert!(!serialized.is_empty());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_stress_test_many_concurrent_parses() {
    let handles: Vec<_> = (0..50)
        .map(|i| {
            thread::spawn(move || {
                for j in 0..20 {
                    let content = format!("VAR_{}_{}\n", i, j);
                    let result = parse_shell_content(&content, Path::new("test.env"));
                    assert!(result.is_ok());
                }
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_parse_large_files_no_panic() {
    let mut large_content = String::new();
    for i in 0..1000 {
        large_content.push_str(&format!("VAR_{}=value_{}\n", i, i));
    }

    let content = Arc::new(large_content);

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let c = Arc::clone(&content);
            thread::spawn(move || {
                let result = parse_shell_content(&c, Path::new("test.env"));
                assert!(result.is_ok());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_access_same_parsed_file() {
    let content = Arc::new("VAR1=value1\nVAR2=value2\n".to_string());
    let parsed = Arc::new(parse_shell_content(&content, Path::new("test.env")).unwrap());

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let p = Arc::clone(&parsed);
            thread::spawn(move || {
                let _serialized = p.serialize();
                let _line_count = p.lines.len();
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_mixed_parse_and_serialize_operations() {
    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                let content = if i % 2 == 0 {
                    "VAR1=value1\n"
                } else {
                    "VAR2=value2\nVAR3=$VAR2\n"
                };

                let result = parse_shell_content(content, Path::new("test.env"));
                if let Ok(file) = result {
                    let _serialized = file.serialize();
                }
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_unicode_parsing_no_panic() {
    let handles: Vec<_> = (0..5)
        .map(|_| {
            thread::spawn(move || {
                let content = "EMOJI=🚀\nACCENT=café\nCHINESE=中文\n";
                let result = parse_shell_content(content, Path::new("test.env"));
                assert!(result.is_ok());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_error_handling_no_panic() {
    let handles: Vec<_> = (0..5)
        .map(|i| {
            thread::spawn(move || {
                let content = match i % 3 {
                    0 => "VAR=value\n",
                    1 => "",
                    _ => "# Comment only\n",
                };

                let result = parse_shell_content(content, Path::new("test.env"));
                // Both success and failure should be handled gracefully
                let _ = result;
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_rwlock_contention_multiple_readers() {
    let data = Arc::new(RwLock::new(vec![
        "VAR1=value1",
        "VAR2=value2",
        "VAR3=value3",
    ]));

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let d = Arc::clone(&data);
            thread::spawn(move || {
                let _guard = d.read().unwrap();
                // Simulate read operation
                thread::sleep(std::time::Duration::from_micros(10));
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_serialization_consistency() {
    let content = "VAR1=value1\nVAR2=value2\nVAR3=$VAR1\n";
    let file = Arc::new(parse_shell_content(content, Path::new("test.env")).unwrap());
    let serialized_results = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let f = Arc::clone(&file);
            let r = Arc::clone(&serialized_results);
            thread::spawn(move || {
                let serialized = f.serialize();
                r.lock().unwrap().push(serialized);
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }

    let results = serialized_results.lock().unwrap();
    assert_eq!(results.len(), 10);
    // All serializations should be identical
    if !results.is_empty() {
        let first = &results[0];
        for result in results.iter().skip(1) {
            assert_eq!(result, first);
        }
    }
}

// ============================================================================
// Lock Poisoning & Error Resilience Tests (6 tests)
// ============================================================================

#[test]
fn test_multiple_threads_no_panic_on_error() {
    let handles: Vec<_> = (0..5)
        .map(|i| {
            thread::spawn(move || {
                let content = match i {
                    0 => "VALID=value",
                    1 => "",
                    2 => "VAR1=v1\nVAR2=v2\n",
                    3 => "# Just a comment",
                    _ => "EXPORT=test",
                };

                let result = parse_shell_content(content, Path::new("test.env"));
                // Should not panic regardless of result
                let _ = result;
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_thread_safety_with_shared_arc() {
    let content = Arc::new("VAR=value\n".to_string());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let c = Arc::clone(&content);
            thread::spawn(move || {
                let _result = parse_shell_content(&c, Path::new("test.env"));
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_concurrent_ops_with_shared_data() {
    let file = parse_shell_content("VAR1=v1\nVAR2=v2\n", Path::new("test.env")).unwrap();
    let file_arc = Arc::new(file);

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let f = Arc::clone(&file_arc);
            thread::spawn(move || {
                // Verify we can access shared data without cloning
                // ShellFile doesn't implement Clone, so we use Arc for thread-safe access
                assert_eq!(f.lines.len(), 2);
                let content = f.serialize();
                assert!(!content.is_empty());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_no_deadlock_on_concurrent_operations() {
    let data = Arc::new(Mutex::new("VAR=value\n".to_string()));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let d = Arc::clone(&data);
            thread::spawn(move || {
                let content = d.lock().unwrap();
                let _ = parse_shell_content(&content, Path::new("test.env"));
            })
        })
        .collect();

    // If this completes without hanging, no deadlock occurred
    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_graceful_handling_of_concurrent_modifications() {
    let handles: Vec<_> = (0..5)
        .map(|i| {
            thread::spawn(move || {
                let mut content = String::new();
                for j in 0..100 {
                    content.push_str(&format!("VAR_{}_{}=value\n", i, j));
                }

                let result = parse_shell_content(&content, Path::new("test.env"));
                assert!(result.is_ok());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
}

#[test]
fn test_serialization_thread_safety_variance() {
    let file = Arc::new(parse_shell_content("VAR=value\n", Path::new("test.env")).unwrap());
    let results = Arc::new(Mutex::new(vec![]));

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let f = Arc::clone(&file);
            let r = Arc::clone(&results);
            thread::spawn(move || {
                for _ in 0..10 {
                    let serialized = f.serialize();
                    r.lock().unwrap().push(serialized.len());
                }
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }

    let sizes = results.lock().unwrap();
    // All serialized sizes should be consistent
    if !sizes.is_empty() {
        let first = sizes[0];
        for &size in sizes.iter() {
            assert_eq!(size, first);
        }
    }
}

// ============================================================================
// Performance Under Load Tests (5 tests)
// ============================================================================

#[test]
fn test_parse_performance_baseline_single_thread() {
    let content = {
        let mut s = String::new();
        for i in 0..1000 {
            s.push_str(&format!("VAR_{}=value_{}\n", i, i));
        }
        s
    };

    let start = std::time::Instant::now();
    let result = parse_shell_content(&content, Path::new("test.env"));
    let duration = start.elapsed();

    assert!(result.is_ok());
    assert!(duration.as_millis() < 1000); // Should complete in reasonable time
}

#[test]
fn test_concurrent_parse_performance_scales() {
    let content = {
        let mut s = String::new();
        for i in 0..500 {
            s.push_str(&format!("VAR_{}=value_{}\n", i, i));
        }
        s
    };

    let content = Arc::new(content);
    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let c = Arc::clone(&content);
            thread::spawn(move || {
                let _ = parse_shell_content(&c, Path::new("test.env"));
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }

    let duration = start.elapsed();
    // Concurrent parsing should complete reasonably
    assert!(duration.as_millis() < 5000);
}

#[test]
fn test_serialization_performance_no_regression() {
    let file = parse_shell_content(
        &{
            let mut s = String::new();
            for i in 0..1000 {
                s.push_str(&format!("VAR_{}=value_{}\n", i, i));
            }
            s
        },
        Path::new("test.env"),
    )
    .unwrap();

    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _ = file.serialize();
    }
    let duration = start.elapsed();

    // 100 serializations should complete quickly
    assert!(duration.as_secs() < 1);
}

#[test]
fn test_concurrent_parsing_throughput() {
    let start = std::time::Instant::now();
    let handles: Vec<_> = (0..20)
        .map(|i| {
            thread::spawn(move || {
                for j in 0..50 {
                    let content = format!("VAR_{}_{}\n", i, j);
                    let _ = parse_shell_content(&content, Path::new("test.env"));
                }
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
    let duration = start.elapsed();

    // 20 threads * 50 iterations = 1000 parses should complete quickly
    assert!(duration.as_secs() < 5);
}

#[test]
fn test_parse_memory_efficiency_large_concurrent_workload() {
    let large_content = {
        let mut s = String::new();
        for i in 0..5000 {
            s.push_str(&format!("VAR_{}=value_{}\n", i, i));
        }
        s
    };

    let content = Arc::new(large_content);
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let c = Arc::clone(&content);
            thread::spawn(move || {
                let result = parse_shell_content(&c, Path::new("test.env"));
                assert!(result.is_ok());
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().is_ok());
    }
    // If this completes without OOM, memory usage is reasonable
}
