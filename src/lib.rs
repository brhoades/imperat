mod resource;
mod step;

// use step::{StepFunction, StepInput, StepParam};
use std::any::TypeId;
use std::sync::Arc;
use step::{Callable, Dep, FromTypeMap, TypeMap};

/// A resource which can be passed into a step.
pub struct Resource<T: Send + Sized + 'static>(T);

pub fn new<O>() -> ImperativeStepBuilder<O> {
    ImperativeStepBuilder::<O>::default()
}

/// a resolved step
struct Step<O> {
    name: String,
    // XXX: allow an arbitrary return type which can imply fallibility if impl'd
    resolved_fn: Box<dyn FnOnce() -> O>,
}

pub struct ImperativeStepBuilder<O> {
    tm: TypeMap,
    steps: Vec<Step<O>>,
    errors: Vec<String>, // TODO: this error
}

impl<O> Default for ImperativeStepBuilder<O> {
    fn default() -> Self {
        ImperativeStepBuilder {
            tm: Default::default(),
            steps: Default::default(),
            errors: Default::default(),
        }
    }
}

impl<O> ImperativeStepBuilder<O> {
    // XXX: allow parallel steps
    pub fn add_step<C: Callable<A, Out = O> + 'static, A: FromTypeMap>(
        mut self,
        name: &str,
        func: C,
    ) -> Self {
        let Some(args) = A::retrieve_from_map(&self.tm) else {
            eprintln!("will not run step '{name}' as at least one dependency was absent");
            return self;
        };
        self.steps.push(Step {
            name: "idk".to_string(),
            resolved_fn: Box::new(move || func.call(args)),
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

    pub fn execute(self) -> Vec<O> {
        let res = self.steps.into_iter().map(|s| (s.resolved_fn)()).collect();
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
    #[test]
    fn test_ordered_exec() {
        fn example_step() -> usize {
            println!("step");
            1
        }

        fn example_step_dep(_db: Dep<TestDatabase>) -> usize {
            println!("step dep");
            2
        }

        let res = ImperativeStepBuilder::default()
            .add_dep(TestDatabase)
            .add_step("example step", example_step)
            .add_step("example step with a dep", example_step_dep)
            .execute();

        assert_eq!(vec![1, 2], res);
    }

    // missing deps should cause a failure and not run
    #[should_panic]
    #[test]
    fn test_missing_deps() {
        fn missing_dep_step(_db: Dep<TestDatabase>) -> usize {
            println!("step dep");
            0
        }

        let res = ImperativeStepBuilder::default()
            .add_dep(TestDatabase)
            .add_step("example step", missing_dep_step)
            .execute();

        assert_eq!(vec![1, 2], res);
    }
}
