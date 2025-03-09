#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
mod builder;
mod callable;

pub use builder::{
    Error as BuilderError, ImperativeStepBuilder, IntoStepOutcome, new as new_builder,
};
pub use callable::Callable;
pub use imperat_common::{Dep, FromTypeMap, TypeMap};
pub use imperat_macros::Dependency;

pub mod prelude {
    pub use super::{
        Callable, Dep, Dependency, ImperativeStepBuilder, IntoStepOutcome,
        new_builder as new_imperative_builder,
    };
}
