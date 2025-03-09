mod outcome;
mod step;

use std::{
    any::TypeId,
    sync::{Arc, Mutex},
};
use thiserror::Error;

use crate::{FromTypeMap, TypeMap, prelude::*};
pub use outcome::IntoStepOutcome;
pub use step::Group;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to resolve at least one dependency in step '{0}'")]
    DepResolution(String),
    #[error("failed to add a dependency of type '{0:?}' as it was already present")]
    AddDep(TypeId),
    #[error("step '{0}' failed to execute: {1}")]
    Step(String, Box<dyn std::error::Error + Send + Sync>),
    #[error("step '{0}' returned a fatal outcome without error")]
    UnknownStep(String),
    #[error("group '{0}' had an error: {1}")]
    Group(String, Box<dyn std::error::Error + Send + Sync>),
}

type Result<T> = std::result::Result<T, Error>;

/// The primary entrypoint to building out an imperative runner. Initialize
/// with default and then chain calls to each other.
#[must_use]
pub fn new<O>() -> ImperativeStepBuilder<O> {
    ImperativeStepBuilder::<O>::default()
}

/// A builder which returns an output `O` on execution. Create one
/// by calling `new`.
pub struct ImperativeStepBuilder<O> {
    tm: Arc<Mutex<TypeMap>>,
    default: Group<O>,
    groups: Vec<Group<O>>,
    errors: Arc<Mutex<Vec<Error>>>,
}

#[allow(clippy::missing_fields_in_debug)]
impl<O> std::fmt::Debug for ImperativeStepBuilder<O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImperativeStepBuilder")
            .field("tm", &self.tm.lock().unwrap())
            .field("errors", &self.errors.lock().unwrap())
            .finish()
    }
}

impl<O> Default for ImperativeStepBuilder<O> {
    fn default() -> Self {
        let tm: Arc<Mutex<TypeMap>> = Arc::default();
        let errors: Arc<Mutex<Vec<Error>>> = Arc::default();

        ImperativeStepBuilder::<O> {
            tm: tm.clone(),
            groups: vec![],
            errors: errors.clone(),
            default: Group::new(tm, errors),
        }
    }
}

impl<O: IntoStepOutcome + 'static> ImperativeStepBuilder<O> {
    // XXX: allow parallel steps
    /// Add a step with the provided name. To the default top-level group.
    /// See `Group::add_step`.
    #[must_use]
    pub fn add_step<C: Callable<A, Out = O> + 'static, A: FromTypeMap>(
        mut self,
        name: &str,
        func: C,
    ) -> Self {
        self.default.add_step(name, func);
        self
    }

    /// Add a dependency with a unique type. Added dependencies can then
    /// be referenced in step arguments by wrapping them in `Dep<T>`.
    ///
    /// All added dependencies must have a unique type or an error will occur.
    /// The type of a dependency is used to inject the dependency into steps.
    ///
    /// # Panics
    /// If the typemap mutex is poisoned.
    #[must_use]
    pub fn add_dep<T: 'static>(self, dep: T) -> Self {
        let mut tm = self.tm.lock().expect("imperat typemap mutex poisoned");
        if tm.get::<T>().is_some() {
            self.default.add_error(Error::AddDep(TypeId::of::<T>()));
            drop(tm);
            return self;
        }
        tm.bind(dep);
        drop(tm);

        self
    }

    /// Execute this runner. All configured steps will be ran.
    /// If any errors occurred during building or while executing,
    /// all executions tops and the error is returned.
    ///
    /// # Panics
    /// If the errors mutex is poisoned.
    pub async fn execute(self) -> Result<Vec<O>> {
        if let Some(e) = self.errors.lock().expect("errors mutex poisoned").pop() {
            return Err(e);
        }

        let mut outputs = vec![];
        let mut groups = vec![self.default];
        groups.extend(self.groups);
        for g in groups {
            let res = g.execute().await?;
            outputs.extend(res);
        }

        Ok(outputs)
    }
}
