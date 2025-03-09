use imperat::{BuilderError, prelude::*};

struct Database;

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
        .add_dep(Database)
        .add_step("example step", example_step)
        .add_step("example step with a dep", example_step_dep)
        .execute()
        .await
        .unwrap();

    assert_eq!(vec![1, 2], res);
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
