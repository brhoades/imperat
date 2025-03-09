use std::{any::TypeId, pin::Pin};
use thiserror::Error;

use crate::{
    callable::Callable,
    dependencies::{Dep, FromTypeMap, TypeMap},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to resolve at least one dependency in step '{0}'")]
    DepResolution(String),
    #[error("failed to add a dependency of type '{0:?}' as it was already present")]
    AddDep(TypeId),
    #[error("step '{0}' failed to execute: {1}")]
    Step(String, Box<dyn std::error::Error>),
    #[error("step '{0}' returned a fatal outcome without error")]
    UnknownStep(String),
}

type Result<T> = std::result::Result<T, Error>;

/// The primary entrypoint to building out an imperative runner. Initialize
/// with default and then chain calls to each other.
#[must_use]
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
    // errors accumulated at build time
    errors: Vec<Error>,
}

impl<O> Default for ImperativeStepBuilder<O> {
    fn default() -> Self {
        ImperativeStepBuilder::<O> {
            tm: TypeMap::default(),
            steps: Vec::default(),
            errors: Vec::default(),
        }
    }
}

impl<O: IntoStepOutcome + 'static> ImperativeStepBuilder<O> {
    // XXX: allow parallel steps
    /// Add a step with the provided name. The passed function arguments must
    /// take only types which implement `FromTypeMap`. Have all arguments wrapped
    /// with `Dep<T>` to pass it to this function.
    ///
    /// If a dependency is not found in builder, an error will be stored. It is
    /// later returned on run.
    #[must_use]
    pub fn add_step<C: Callable<A, Out = O> + 'static, A: FromTypeMap>(
        mut self,
        name: &str,
        func: C,
    ) -> Self {
        let Some(args) = A::retrieve_from_map(&self.tm) else {
            eprintln!("will not run step '{name}' as at least one dependency was absent");
            self.errors.push(Error::DepResolution(name.to_string()));
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
    #[must_use]
    pub fn add_dep<T: 'static>(mut self, dep: T) -> Self {
        if self.tm.get::<Dep<T>>().is_some() {
            self.errors.push(Error::AddDep(TypeId::of::<T>()));
            return self;
        }
        self.tm.bind(Dep::new(dep));

        self
    }

    /// Execute this runner. All configured steps will be ran.
    /// If any errors occurred during building or while executing,
    /// all executions tops and the error is returned.
    pub async fn execute(mut self) -> Result<Vec<O>> {
        if let Some(e) = self.errors.pop() {
            return Err(e);
        }
        let mut outputs = Vec::with_capacity(self.steps.len());

        for step in self.steps {
            let r = step.fut.await;
            if r.success() {
                outputs.push(r);
            } else if let Some(e) = r.error() {
                return Err(Error::Step(step.name, e));
            } else {
                return Err(Error::UnknownStep(step.name));
            }
        }
        Ok(outputs)
    }
}

/// All step functions must return a result with a result that
/// dictates step outcome.
///
/// A step whose outcome is not a success halts execution.
/// Not all failures have a matching error.
pub trait IntoStepOutcome {
    /// Returns the error from the step execution, if any.
    fn error(self) -> Option<Box<dyn std::error::Error>>;

    /// Return whether this step succeeded.
    fn success(&self) -> bool;
}

// Nightly:
// an unfailable step, compiler error occurs if a failure is attempted
// pub type Infallible = !;

impl IntoStepOutcome for std::io::Error {
    fn error(self) -> Option<Box<dyn std::error::Error>> {
        Some(Box::new(self))
    }

    fn success(&self) -> bool {
        false
    }
}

impl IntoStepOutcome for Box<dyn std::error::Error> {
    fn error(self) -> Option<Box<dyn std::error::Error>> {
        Some(self)
    }

    fn success(&self) -> bool {
        false
    }
}

impl IntoStepOutcome for bool {
    fn error(self) -> Option<Box<dyn std::error::Error>> {
        None
    }

    fn success(&self) -> bool {
        *self
    }
}

impl<T, E: IntoStepOutcome + Into<Box<dyn std::error::Error>>> IntoStepOutcome
    for std::result::Result<T, E>
{
    fn error(self) -> Option<Box<dyn std::error::Error>> {
        if self.is_err() {
            self.err().map(Into::into)
        } else {
            None
        }
    }

    fn success(&self) -> bool {
        self.is_ok()
    }
}

// Enable blanket implementations for primitives which never fail.
macro_rules! impl_into_step_outcome {
    ($($typ:ty)*) => {
        $(
          impl IntoStepOutcome for $typ {
              fn error(self) -> Option<Box<dyn std::error::Error>> {
                  None
              }

              fn success(&self) -> bool {
                  true
              }
          }
        )*
    };
}

impl_into_step_outcome!(
    () usize isize char &str String u8 i8 i16 u16 i32 u32
    i64 u64 i128 u128 f32 f64
);
