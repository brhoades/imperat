use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::Deref,
    sync::Arc,
};
use variadics_please::all_tuples;

// https://nickbryan.co.uk/software/using-a-type-map-for-dependency-injection-in-rust/
#[derive(Default)]
pub struct TypeMap {
    bindings: HashMap<TypeId, Box<dyn Any>>,
}

impl TypeMap {
    pub fn new() -> Self {
        TypeMap {
            bindings: HashMap::default(),
        }
    }

    pub fn bind<T: Any>(&mut self, val: T) {
        self.bindings.insert(val.type_id(), Box::new(val));
    }

    pub fn get<T: Any>(&self) -> Option<&T> {
        self.bindings
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }
}

pub trait FromTypeMap: Any + Sized {
    fn retrieve_from_map(tm: &TypeMap) -> Option<Self>;
}

pub trait Callable<Args: FromTypeMap> {
    type Out;

    fn call(&self, args: Args) -> Self::Out;
}

// fans out an implementation for 0 to 16-tuple of generics of Callable
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
        impl<Func, O, $($param: FromTypeMap),*> Callable<($($param,)*)> for Func
            where Func: Fn($($param,)*) -> O
        {
            type Out = O;

            #[inline]
            fn call(&self, ($($param,)*): ($($param,)*)) -> Self::Out {
                (self)($($param,)*)
            }

        }
    }
}

all_tuples!(impl_callable_tuples, 0, 16, F);

// fans out an implementation for 0 to 16-tuple of generics of FromTypeMap
macro_rules! impl_fromtypemap_tuples {
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
        impl<$($param: FromTypeMap,)*> FromTypeMap for ($($param,)*) {
            fn retrieve_from_map(tm: &TypeMap) -> Option<Self> {
                (
                    Some(($(
                        $param::retrieve_from_map(tm)?,
                    )*))
                )
            }
        }
    }
}

all_tuples!(impl_fromtypemap_tuples, 0, 16, F);

pub struct Dep<T: ?Sized>(Arc<T>);

impl<T> Dep<T> {
    pub fn new(val: T) -> Dep<T> {
        Dep(Arc::new(val))
    }

    fn get(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> Clone for Dep<T> {
    fn clone(&self) -> Self {
        Dep(self.0.clone())
    }
}

impl<T: ?Sized> Deref for Dep<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Arc<T> {
        &self.0
    }
}

impl<T: ?Sized + 'static> FromTypeMap for Dep<T> {
    fn retrieve_from_map(tm: &TypeMap) -> Option<Self> {
        tm.get::<Self>().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Database;
    #[derive(Debug)]
    struct Config(i32, u32);

    #[test]
    fn test_retrieval() {
        let mut tm = TypeMap::new();

        tm.bind(Dep::new(Database));
        tm.bind(Dep::new(Config(2, 3)));

        tm.get::<Dep<Database>>().unwrap();
        let cfg = tm.get::<Dep<Config>>().unwrap();

        assert_eq!(cfg.get().0, 2);
        assert_eq!(cfg.get().1, 3);
    }
}
