# Imperat
Imperat is a Latin verb that translates to "commands." Imperat is inspired by [bevy](https://bevyengine.org/)'s dependency injection. Impereat enables step-by-step execution of configured functions and handles dependencies for you.

Managing a large set of discrete tasks, their dependencies, and any failures isn't hard, but refactoring those tasks or dependences is. Imperat helps: using the type of dependencies, Imperat automatically plumbs dependencies into each task and then executes the task. If any task fails, execution stops.

Imperat is early in development. Expect APIs to change.

## What's next?
A loose roadmap for features includes:
  * Configurable failure toleration
  * Parallel execution support
  * Task execution reports
  * Retries

## Installation
`cargo add imperat` or add to your `Cargo.toml`:
```toml
imperat = "0.1.0"
```

## Example
Additional examples can be found in [integration tests](./crates/imperat/tests/integration_tests.rs).


The example below shows basic dependency injection across a handful of functions of different shapes.

```rust
use imperat::prelude::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// fake database connection pool
#[derive(Clone, Dependency)]
struct Database;

// fake configuration option
#[derive(Clone, Dependency)]
struct Config(bool);

async fn delete_dogs_table(db: Database) -> Result<()> {
    db.exec("DELETE FROM dogs;").await.map_err(Box::new)
}

let res = new_imperative_builder()
  .add_dep(Database)
  .add_dep(Config(true))
  .add_step("delete dogs table", delete_dogs_table)
  .add_step("add dogs if configured", async |db: Database, config: Config| {
      if cfg.0 {
        db
          .exec("INSERT INTO dogs (breed, spots) VALUES ('Pembroke Welsh Corgi', true);")
          .await
          .map_err(Box::new)
      } else {
          Ok(())
      }
    })
  .execute()
  .await;

match res {
    Ok(res) => {
        println!("all steps finished");
        for (i, r) in res.into_iter().enumerate() {
            println!("step #{i} completed with {r:?}");
        }
    },
    Err(e) => eprintln!("at least one step failed: {e:?}"),
}
```


## Features
`anyhow`: enable built-in `IntoStepOutcome` support for `anyhow::Error`.
