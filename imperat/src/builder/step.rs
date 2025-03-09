use super::{Error, IntoStepOutcome, Result};
use crate::{FromTypeMap, TypeMap, prelude::*};
use futures::{StreamExt, stream::FuturesOrdered};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};

/// A resolved step which is ready to be ran.
pub struct Step<O> {
    #[allow(dead_code)]
    name: String,
    // XXX: allow an arbitrary return type which can imply fallibility if impl'd
    fut: Pin<Box<dyn Future<Output = O>>>,
}

#[derive(Default)]
struct GroupOptions {
    parallel: bool,
    tolerate_failure: bool,
}

/// A logical group of steps. Every builder contains an implicit starting group
/// of steps. Subgroups allow specific steps to have some behavior.
pub struct Group<O> {
    tm: Arc<Mutex<TypeMap>>,
    steps: Vec<Step<O>>,
    // errors accumulated at build time
    errors: Arc<Mutex<Vec<Error>>>,
    opts: GroupOptions,
}

impl<O> Group<O> {
    pub(super) fn new(tm: Arc<Mutex<TypeMap>>, errors: Arc<Mutex<Vec<Error>>>) -> Self {
        Self {
            steps: vec![],
            errors,
            tm,
            opts: GroupOptions::default(),
        }
    }

    pub(super) fn add_error(&self, e: Error) {
        self.errors
            .lock()
            .expect("imperat group mutex poisoned")
            .push(e);
    }
}

impl<O: IntoStepOutcome + 'static> Group<O> {
    /// Adds a step to this group.
    pub(super) fn add_step<C: Callable<A, Out = O> + 'static, A: FromTypeMap>(
        &mut self,
        name: &str,
        func: C,
    ) {
        let Some(args) =
            A::retrieve_from_map(&self.tm.lock().expect("imperat typemap mutex poisoned"))
        else {
            eprintln!("will not run step '{name}' as at least one dependency was absent");
            self.add_error(Error::DepResolution(name.to_string()));
            return;
        };
        self.steps.push(Step {
            name: name.to_string(),
            fut: Box::pin(func.call(args)),
        });
    }

    /// Execute this group, returning all of the results.
    pub(super) async fn execute(self) -> Result<Vec<O>> {
        let mut outputs = Vec::with_capacity(self.steps.len());

        // implies tolerate_failure for now. We'd need something special
        // here to allow a single failure to interrupt all futures.
        if self.opts.parallel {
            return Ok(self
                .steps
                .into_iter()
                .map(|s| s.fut)
                .collect::<FuturesOrdered<_>>()
                .collect()
                .await);
        }

        for step in self.steps {
            let r = step.fut.await;
            if self.opts.tolerate_failure {
                outputs.push(r);
                continue;
            }

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

/// Allows incrementally building groups with specific options.
pub struct GroupBuilder<O>(pub(super) Group<O>);

impl<O: IntoStepOutcome + 'static> GroupBuilder<O> {
    pub(super) fn new(tm: Arc<Mutex<TypeMap>>, errors: Arc<Mutex<Vec<Error>>>) -> Self {
        GroupBuilder(Group::new(tm, errors))
    }

    /// Add a step with this name to the provided group.
    pub fn add_step<C: Callable<A, Out = O> + 'static, A: FromTypeMap>(
        mut self,
        name: &str,
        func: C,
    ) -> Self {
        self.0.add_step(name, func);
        self
    }

    /// Run all the steps in this group in parallel. Currently,
    /// this implies `GroupOptions::tolerate_failure` but that may change in the future;
    /// set both if both are desired.
    pub fn parallel(mut self) -> Self {
        self.0.opts.parallel = true;
        self.tolerate_failure()
    }

    /// Don't exit on the first failure.
    pub fn tolerate_failure(mut self) -> Self {
        self.0.opts.tolerate_failure = true;
        self
    }
}
