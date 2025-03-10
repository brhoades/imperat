use super::{Error, IntoStepOutcome, Result};
use crate::{FromTypeMap, TypeMap, prelude::*};
use futures::{StreamExt, stream::FuturesOrdered};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Mutex},
};

/// A resolved step which is ready to be ran.
pub struct Step<O> {
    name: String,
    fut: Pin<Box<dyn Future<Output = O>>>,
}

impl<O> Step<O> {
    /// Returns the name of this step.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Options which apply to a group and its steps.
struct GroupOptions<O> {
    parallel: bool,
    tolerate_failure: bool,
    callbacks: Vec<CallbackKind<O>>,
}

impl<O> Default for GroupOptions<O> {
    fn default() -> Self {
        Self {
            parallel: false,
            tolerate_failure: false,
            callbacks: vec![],
        }
    }
}

pub type BeforeCallbackFn<O> = dyn Fn(&Step<O>);
pub type AfterCallbackFn<O> = dyn Fn(&str, &O);

/// A variant of a callback on a group.
pub(super) enum CallbackKind<O> {
    /// Called before the step executes on the step.
    BeforeStep(Arc<BeforeCallbackFn<O>>),
    /// Called after the step executes. Is passed the step's
    /// name and result.
    AfterStep(Arc<AfterCallbackFn<O>>),
}

// derive fails for some reason
impl<O> Clone for CallbackKind<O> {
    fn clone(&self) -> Self {
        match self {
            CallbackKind::BeforeStep(cb) => CallbackKind::BeforeStep(cb.clone()),
            CallbackKind::AfterStep(cb) => CallbackKind::AfterStep(cb.clone()),
        }
    }
}

/// A logical group of steps. Every builder contains an implicit starting group
/// of steps. Subgroups allow specific steps to have some behavior.
pub struct Group<O> {
    tm: Arc<Mutex<TypeMap>>,
    steps: Vec<Step<O>>,
    // errors accumulated at build time
    errors: Arc<Mutex<Vec<Error>>>,
    opts: GroupOptions<O>,
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

    /// Internal API to add a callback to this group.
    pub(super) fn add_callback(&mut self, cb: CallbackKind<O>) {
        self.opts.callbacks.push(cb);
    }

    /// Internal API to read callbacks from this group.
    pub(super) fn callbacks(&self) -> &[CallbackKind<O>] {
        &self.opts.callbacks
    }

    /// Execute this group, returning all of the results. The results
    /// are grouped by the step name. The last defined with a duplicate
    /// step name will appear in the results.
    pub(super) async fn execute(self) -> Result<HashMap<String, O>> {
        let mut outputs = HashMap::with_capacity(self.steps.len());

        let exec_step = async |s, cbs: &[CallbackKind<O>]| {
            for cb in cbs {
                if let CallbackKind::BeforeStep(cb) = cb {
                    cb(&s);
                }
            }
            let Step { name, fut } = s;
            let res = fut.await;
            for cb in cbs {
                if let CallbackKind::AfterStep(cb) = cb {
                    cb(&name, &res);
                };
            }
            res
        };

        let cbs = self.callbacks().to_vec();
        // implies tolerate_failure for now. We'd need something special
        // here to allow a single failure to interrupt all futures.
        if self.opts.parallel {
            return Ok(self
                .steps
                .into_iter()
                .map(|s| async { (s.name.clone(), exec_step(s, &cbs).await) })
                .collect::<FuturesOrdered<_>>()
                .collect()
                .await);
        }

        for step in self.steps {
            let name = step.name.clone();
            let r = exec_step(step, &cbs).await;
            if self.opts.tolerate_failure {
                outputs.insert(name, r);
                continue;
            }

            if r.success() {
                outputs.insert(name, r);
            } else if let Some(e) = r.error() {
                return Err(Error::Step(name, e));
            } else {
                return Err(Error::UnknownStep(name));
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

    /// Pass a callback to run for this group before every step.
    pub fn before_step(mut self, cb: impl Fn(&Step<O>) + 'static) -> Self {
        self.0
            .opts
            .callbacks
            .push(CallbackKind::BeforeStep(Arc::new(cb)));
        self
    }

    /// Pass a callback to run for this group after every step.
    pub fn after_step(mut self, cb: impl Fn(&str, &O) + 'static) -> Self {
        self.0
            .opts
            .callbacks
            .push(CallbackKind::AfterStep(Arc::new(cb)));
        self
    }
}
