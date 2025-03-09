use crate::dependencies::FromTypeMap;
use variadics_please::all_tuples;

/// Something that is callable with a specific interface.
#[async_trait::async_trait]
pub trait Callable<Args: FromTypeMap> {
    type Out;

    async fn call(self, args: Args) -> Self::Out;
}

// Fans out an implementation for 0 to 16-tuple of generics of Callable.
// Allows the crate to take tuples of arguments resolved elsewhere and then
// use that tuple to call a function.
macro_rules! impl_callable_tuples {
    ($($param: ident),*) => {
        #[allow(
            non_snake_case,
            reason = "Certain variable names are provided by the caller, not by us."
        )]
        #[allow(
            unused_variables,
            reason = "Zero-length tuples won't use some of the parameters."
        )]
        #[expect(
            clippy::allow_attributes,
            reason = "This is in a macro, and as such, the below lints may not always apply."
        )]
        #[async_trait::async_trait]
        impl<Func, Fut, O, $($param: FromTypeMap + Send + Sync),*> Callable<($($param,)*)> for Func
        where Func: Fn($($param,)*) -> Fut + Send + Sync,
              Fut: Future<Output = O> + Send,

        {
            type Out = O;

            #[inline]
            async fn call(self, ($($param,)*): ($($param,)*)) -> Self::Out {
                (self)($($param,)*).await
            }

        }
    }
}

all_tuples!(impl_callable_tuples, 0, 16, F);
