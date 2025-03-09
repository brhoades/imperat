/// All step functions must return a result with a result that
/// dictates step outcome.
///
/// A step whose outcome is not a success halts execution.
/// Not all failures have a matching error.
pub trait IntoStepOutcome {
    /// Returns the error from the step execution, if any.
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>>;

    /// Return whether this step succeeded.
    fn success(&self) -> bool;
}

// Nightly:
// an unfailable step, compiler error occurs if a failure is attempted
// pub type Infallible = !;

impl IntoStepOutcome for std::io::Error {
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        Some(Box::new(self))
    }

    fn success(&self) -> bool {
        false
    }
}

impl IntoStepOutcome for Box<dyn std::error::Error + Send + Sync> {
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        Some(self)
    }

    fn success(&self) -> bool {
        false
    }
}

impl IntoStepOutcome for bool {
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        None
    }

    fn success(&self) -> bool {
        *self
    }
}

#[cfg(feature = "anyhow")]
impl IntoStepOutcome for anyhow::Error {
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        Some(self.into())
    }

    fn success(&self) -> bool {
        false
    }
}

impl<T, E: IntoStepOutcome + Into<Box<dyn std::error::Error + Send + Sync>>> IntoStepOutcome
    for std::result::Result<T, E>
{
    fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
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
              fn error(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
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
