use imperat::{BuilderError, prelude::*};
use std::{
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::time::sleep;

struct Database;
#[derive(Clone, Dependency)]
struct DeriveDataSource;

// ordered exec should run steps in order
#[tokio::test]
async fn test_ordered_exec() {
    async fn example_step() -> usize {
        println!("step");
        1
    }

    async fn example_step_dep(_db: Dep<Database>) -> usize {
        println!("step dep");
        2
    }

    let res = new_imperative_builder()
        .add_dep(Dep::new(Database))
        .add_step("example step", example_step)
        .add_step("example step with a dep", example_step_dep)
        .execute()
        .await
        .unwrap();

    let mut vs: Vec<_> = res.values().collect();
    vs.sort();
    assert_eq!(vec![&1, &2], vs);
}

// a derive Dependency struct should resolve fine
#[tokio::test]
async fn test_derive_dependency() {
    let b = new_imperative_builder().add_dep(DeriveDataSource).add_step(
        "step 1",
        async |_: DeriveDataSource| {
            println!("foo");
            1
        },
    );
    println!("{b:?}");

    let res = b.execute().await.unwrap();

    let mut vs: Vec<_> = res.values().collect();
    vs.sort();
    assert_eq!(vec![&1], vs);
}

// missing deps should error out.
#[tokio::test]
async fn test_missing_deps() {
    async fn missing_dep_step(_db: Dep<Database>) -> usize {
        println!("step dep");
        0
    }

    let e = new_imperative_builder()
        .add_step("example step", missing_dep_step)
        .execute()
        .await
        .expect_err("should have failed");
    assert!(matches!(e, BuilderError::DepResolution(_)), "{e:?}");
}

#[derive(Error, Debug, PartialEq, Eq)]
enum Error {
    #[error("uhoh")]
    TestOne,
}

impl IntoStepOutcome for Error {
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        Some(Box::new(self))
    }

    fn success(&self) -> bool {
        false
    }
}

// a step with an error should yield an error on execute
#[tokio::test]
async fn fail_step_yields_error() {
    async fn fail_step() -> std::result::Result<(), Error> {
        Err(Error::TestOne)
    }

    let name = "fatal step".to_string();
    let e = new_imperative_builder()
        .add_step(&name, fail_step)
        .execute()
        .await
        .expect_err("should have failed");

    match e {
        BuilderError::Step(msg, e) => {
            assert!(
                e.downcast::<Error>().ok() == Some(Box::new(Error::TestOne)),
                "error msg: {msg:?}",
            )
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// a step with an error should not execute other steps
#[tokio::test]
async fn fail_step_stops_execution() {
    static CNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));
    new_imperative_builder()
        .add_step("one", async move || {
            CNT.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .add_step("two", async || Err(Error::TestOne))
        .add_step("three", async || {
            CNT.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .add_step("four", async || {
            CNT.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .execute()
        .await
        .expect_err("should have failed");

    assert_eq!(CNT.load(Ordering::Relaxed), 1);
}

// A parallel group should run all steps in parallel.
#[tokio::test]
async fn test_parallel_steps_run_in_parallel() {
    let b = new_imperative_builder().new_group(|mut gb| {
        for i in 0..50 {
            gb = gb.add_step(&format!("step #{i}"), async || {
                sleep(Duration::from_millis(10)).await;
            });
        }
        gb.parallel()
    });

    let st = Instant::now();
    let _ = b.execute().await;
    let total = st.elapsed();

    // even though each one waits 10ms, it shouldn't take long.
    // overhead for the single threaded test executor / noisy parallel tests.
    assert!(
        total > Duration::from_millis(10),
        "unexpectedly fast test: {total:?}",
    );
    assert!(
        total < Duration::from_millis(25),
        "total elapsed: {total:?}",
    );
}

// A group that tolerates failure should ignore an individual failure.
#[tokio::test]
async fn test_tolerate_failure() {
    let res = new_imperative_builder()
        .new_group(|mut gb| {
            for i in 0..50 {
                if i % 2 == 0 {
                    gb = gb.add_step(&format!("{i}"), async || true);
                } else {
                    gb = gb.add_step(
                        &format!("{i}"),
                        async || false, // IntoStepOutcome treats false as failure
                    );
                }
            }
            gb.tolerate_failure()
        })
        .execute()
        .await
        .unwrap();

    for (i, (name, r)) in res.into_iter().enumerate() {
        let name = name.parse::<i32>().unwrap();
        if name % 2 == 0 {
            assert!(r, "{i} was not true");
        } else {
            assert!(!r, "{i} was not false");
        }
    }
}

// Callbacks should run before and after steps as configured.
#[tokio::test]
async fn test_callbacks_run() {
    static BEFORE_CNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));
    static AFTER_CNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

    new_imperative_builder()
        .new_group(|mut gb| {
            for i in 0..10 {
                gb = gb.add_step(&format!("step #{i}"), async || {
                    println!("step running");
                });
            }
            gb.before_step(|s| {
                println!("{}: before step", s.name());
                BEFORE_CNT.fetch_add(1, Ordering::Relaxed);
            })
            .after_step(|name, res| {
                println!("{}: after step w/ res {res:?}", name);
                AFTER_CNT.fetch_add(1, Ordering::Relaxed);
            })
        })
        .execute()
        .await
        .unwrap();

    assert_eq!(BEFORE_CNT.load(Ordering::Relaxed), 10);
    assert_eq!(AFTER_CNT.load(Ordering::Relaxed), 10);
}

// Callbacks registered on the top-level builder should apply to
// groups and top-level steps.
#[tokio::test]
async fn test_callbacks_propagate_and_run() {
    static CNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

    let mut b = new_imperative_builder()
        .before_step(|s| {
            println!("before step {}", s.name());
            CNT.fetch_add(1, Ordering::Relaxed);
        })
        .after_step(|n, _| {
            println!("after step: {n}");
            CNT.fetch_add(1, Ordering::Relaxed);
        });

    for gi in 0..5 {
        b = b.new_group(|mut gb| {
            for i in 0..10 {
                gb = gb.add_step(&format!("step #{gi}:#{i}"), async || {
                    println!("step running");
                });
            }
            gb
        });
    }
    for i in 0..10 {
        b = b.add_step(&format!("step ::#{i}"), async || {
            println!("step running");
        });
    }

    b.execute().await.unwrap();
    assert_eq!(CNT.load(Ordering::Relaxed), (5 * 10 + 10) * 2);
}
