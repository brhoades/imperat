use crate::{
    callable::Callable,
    dependencies::{Dep, FromTypeMap, TypeMap},
};
use std::{any::TypeId, pin::Pin};

/// The primary entrypoint to building out an imperative runner. Initialize
/// with default and then chain calls to each other.
pub fn new<O>() -> ImperativeStepBuilder<O> {
    ImperativeStepBuilder::<O>::default()
}

/// A resolved step which is ready to be ran.
struct Step<O> {
    #[allow(dead_code)]
    name: String,
    // XXX: allow an arbitrary return type which can imply fallibility if impl'd
    fut: Pin<Box<dyn Future<Output = O>>>,
}

/// A builder which returns an output `O` on execution. Create one
/// by calling `new`.
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
    /// Add a step with the provided name. The passed function arguments must
    /// take only types which implement `FromTypeMap`. Have all arguments wrapped
    /// with `Dep<T>` to pass it to this function.
    ///
    /// If a dependency is not found in builder, an error will be stored. It is
    /// later returned on run.
    pub fn add_step<C: Callable<A, Out = O> + 'static, A: FromTypeMap>(
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

    /// Add a dependency with a unique type. Added dependencies can then
    /// be referenced in step arguments by wrapping them in `Dep<T>`.
    ///
    /// All added dependencies must have a unique type or an error will occur.
    /// The type of a dependency is used to inject the dependency into steps.
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

    /// Execute this runner. All configured steps will be ran.
    /// If an error occurs, it will be output immediately.
    pub async fn execute(self) -> Vec<O> {
        let mut res = Vec::with_capacity(self.steps.len());
        if !self.errors.is_empty() {
            panic!("{:?}", self.errors);
        }

        for step in self.steps {
            res.push(step.fut.await);
            if !self.errors.is_empty() {
                panic!("{:?}", self.errors);
            }
        }
        res
    }
}
