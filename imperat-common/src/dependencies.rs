use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::Deref,
    sync::Arc,
};
use variadics_please::all_tuples;

/// Nearly 1-to-1 with this blog:
/// <https://nickbryan.co.uk/software/using-a-type-map-for-dependency-injection-in-rust/>
/// A `TypeMap` uniquely stores an arbitrary value by its type. No types
/// can store more than one value.
#[derive(Default, Debug)]
pub struct TypeMap {
    bindings: HashMap<TypeId, Box<dyn Any>>,
}

impl TypeMap {
    /// Creates a new, empty type map.
    pub fn new() -> Self {
        TypeMap::default()
    }

    /// Binds the given value to its type in the type map. If an
    /// existing value for this type exists, it's returned. An existing value
    /// with an incorrect type is returned as none.
    pub fn bind<T: Any>(&mut self, val: T) -> Option<Box<T>> {
        self.bindings
            .insert(val.type_id(), Box::new(val))
            .and_then(|v| v.downcast().ok())
    }

    /// Returns the value in this type map for this unique type.
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.bindings
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }
}

/// A type which can be retrieved from a type map. Its type signature
/// uniquely stores the type in the map.
pub trait FromTypeMap: Any + Sized {
    fn retrieve_from_map(tm: &TypeMap) -> Option<Self>;
}

// Fans out an implementation for 0 to 16-tuple of generics of FromTypeMap. Allows
// the crate to treat a tuple of arguments as individiual arguments to look up
// in a type map. Without this, we'd look up all unique argument as a tuple on
// a function call when resolving dependencies.
//
// In effect, this is a big part of where the magic happens.
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

/// A dependency which can be automatically resolved at runtime
/// by its unique type.
pub struct Dep<T: ?Sized>(Arc<T>);

impl<T> Dep<T> {
    /// Create a new dependency for injection.
    pub fn new(val: T) -> Dep<T> {
        Dep(Arc::new(val))
    }

    /// Yields the inner dependency, destroying the outer wrapper.
    #[must_use]
    pub fn inner(self) -> Arc<T> {
        self.0
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

    // retrieval should get back what it puts in
    #[test]
    fn test_retrieval() {
        let mut tm = TypeMap::new();

        tm.bind(Dep::new(Database));
        tm.bind(Dep::new(Config(2, 3)));

        tm.get::<Dep<Database>>().unwrap();
        let cfg = tm.get::<Dep<Config>>().unwrap();

        // since we're in this module, we have to navigate the
        // private internals of Dep
        assert_eq!(cfg.0.0, 2);
        assert_eq!(cfg.0.1, 3);
    }

    // unset values should be absent
    #[test]
    fn test_missing() {
        let tm = TypeMap::new();
        assert!(tm.get::<Dep<i32>>().is_none());
    }
}
