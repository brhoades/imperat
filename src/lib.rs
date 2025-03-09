#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
mod builder;
mod callable;
mod dependencies;

pub use builder::{Error as BuilderError, ImperativeStepBuilder, new as new_builder};
pub use callable::Callable;
pub use dependencies::Dep;

pub mod prelude {
    pub use super::{Callable, Dep, ImperativeStepBuilder, new_builder as new_imperative_builder};
}
