#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
mod builder;
mod callable;

pub use ::common::{Dep, FromTypeMap, TypeMap};
pub use builder::{
    Error as BuilderError, ImperativeStepBuilder, IntoStepOutcome, new as new_builder,
};
pub use callable::Callable;
pub use macros::Dependency;

pub mod prelude {
    pub use super::{
        Callable, Dep, Dependency, ImperativeStepBuilder, IntoStepOutcome,
        new_builder as new_imperative_builder,
    };
}
