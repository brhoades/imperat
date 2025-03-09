use impereat::{BuilderError, prelude::*};
use std::sync::{
    LazyLock,
    atomic::{AtomicUsize, Ordering},
};
use thiserror::Error;

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

    assert_eq!(vec![1, 2], res);
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

    assert_eq!(vec![1], res);
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
