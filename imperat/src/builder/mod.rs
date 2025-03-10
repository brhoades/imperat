mod outcome;
mod step;

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, Mutex},
};
use thiserror::Error;

use crate::{FromTypeMap, TypeMap, prelude::*};
pub use outcome::IntoStepOutcome;
pub use step::{Group, GroupBuilder, Step};

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

    /// Pass a closure to define a group. The closure operates on a `step::GroupBuilder`.
    /// Return the group builder when done and the group will be added.
    #[must_use]
    pub fn new_group(mut self, new_fn: impl Fn(GroupBuilder<O>) -> GroupBuilder<O>) -> Self {
        let gb = new_fn(GroupBuilder::new(self.tm.clone(), self.errors.clone()));
        // I've decided to not include a finalize() fn on GroupBuilder to avoid
        // confusion when in the closure.
        self.groups.push(gb.0);
        self
    }

    /// Adds a before step callback to top-level steps and all groups.
    /// Callbacks added by this method run after group-specific callbacks,
    /// though this is subject to change.
    #[must_use]
    pub fn before_step(mut self, cb: impl Fn(&Step<O>) + 'static) -> Self {
        self.default
            .add_callback(step::CallbackKind::BeforeStep(Arc::new(cb)));
        self
    }

    /// Adds an after step callback to top-level steps and all groups.
    /// Callbacks added by this method run after group-specific callbacks,
    /// though this is subject to change.
    #[must_use]
    pub fn after_step(mut self, cb: impl Fn(&str, &O) + 'static) -> Self {
        self.default
            .add_callback(step::CallbackKind::AfterStep(Arc::new(cb)));
        self
    }

    /// Execute this runner. All configured groups and steps will be ran.
    /// If any errors occurred during building or while executing,
    /// all execution stops (unless otherwise configured) and the error is returned.
    ///
    /// The returned `HashMap` contains all results by their step name. In the case of
    /// duplicate names, results for the last step by order definition order will
    /// win.
    ///
    /// # Panics
    /// If the errors mutex is poisoned.
    pub async fn execute(mut self) -> Result<HashMap<String, O>> {
        if let Some(e) = self.errors.lock().expect("errors mutex poisoned").pop() {
            return Err(e);
        }

        // The default group's callbacks apply to every child group.
        // For consistency, we populate those callbacks here so that
        // every subgroup gets them last.
        let cbs = self.default.callbacks();
        for group in &mut self.groups {
            for cb in cbs {
                group.add_callback(cb.clone());
            }
        }

        let mut outputs = vec![];
        let mut groups = vec![self.default];
        groups.extend(self.groups);
        for g in groups {
            let res = g.execute().await?;
            outputs.push(res);
        }

        Ok(outputs.into_iter().flatten().collect())
    }
}
