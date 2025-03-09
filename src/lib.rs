mod step;

use std::{any::TypeId, pin::Pin};
use step::{Callable, Dep, FromTypeMap, TypeMap};

/// A resource which can be passed into a step.
pub struct Resource<T: Send + Sized + 'static>(T);

pub fn new<O>() -> ImperativeStepBuilder<O> {
    ImperativeStepBuilder::<O>::default()
}

/// a resolved step
struct Step<O> {
    #[allow(dead_code)]
    name: String,
    // XXX: allow an arbitrary return type which can imply fallibility if impl'd
    fut: Pin<Box<dyn Future<Output = O>>>,
}

pub struct ImperativeStepBuilder<O> {
    tm: TypeMap,
    steps: Vec<Step<O>>,
    errors: Vec<String>, // TODO: this error
}

impl<O> Default for ImperativeStepBuilder<O> {
    fn default() -> Self {
        ImperativeStepBuilder::<O> {
            tm: Default::default(),
            steps: Default::default(),
            errors: Default::default(),
        }
    }
}

impl<O: 'static> ImperativeStepBuilder<O> {
    // XXX: allow parallel steps
    pub fn add_step<C: for<'a> Callable<A, Out = O> + 'static, A: FromTypeMap>(
        mut self,
        name: &str,
        func: C,
    ) -> Self {
        let Some(args) = A::retrieve_from_map(&self.tm) else {
            eprintln!("will not run step '{name}' as at least one dependency was absent");
            self.errors.push(format!(
                "step '{name}' did not run as a dependency was absent",
            ));
            return self;
        };
        self.steps.push(Step {
            name: name.to_string(),
            fut: Box::pin(func.call(args)),
        });
        self
    }

    pub fn add_dep<T: 'static>(mut self, dep: T) -> Self {
        if self.tm.get::<Dep<T>>().is_some() {
            self.errors.push(format!(
                "a dependency of type '{:?}' could not be added as it was already present",
                TypeId::of::<T>(),
            ));
            return self;
        }
        self.tm.bind(Dep::new(dep));

        self
    }

    pub async fn execute(self) -> Vec<O> {
        let mut res = Vec::with_capacity(self.steps.len());
        for step in self.steps {
            res.push(step.fut.await);
        }
        if !self.errors.is_empty() {
            panic!("{:?}", self.errors);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDatabase;

    // ordered exec should run steps in order
    #[tokio::test]
    async fn test_ordered_exec() {
        async fn example_step() -> usize {
            println!("step");
            1
        }

        async fn example_step_dep(_db: Dep<TestDatabase>) -> usize {
            println!("step dep");
            2
        }

        let res = ImperativeStepBuilder::default()
            .add_dep(TestDatabase)
            .add_step("example step", example_step)
            .add_step("example step with a dep", example_step_dep)
            .execute()
            .await;

        assert_eq!(vec![1, 2], res);
    }

    // missing deps should cause a failure and not run
    #[should_panic]
    #[tokio::test]
    async fn test_missing_deps() {
        async fn missing_dep_step(_db: Dep<TestDatabase>) -> usize {
            println!("step dep");
            0
        }

        ImperativeStepBuilder::default()
            .add_step("example step", missing_dep_step)
            .execute()
            .await;
    }
}
