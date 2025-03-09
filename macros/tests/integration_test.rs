//! Compilation tests for cases which did not always work.
#![allow(dead_code)]

use macros::Dependency;

#[derive(Clone, Debug, Dependency)]
struct Bare;

#[derive(Clone, Debug, Dependency)]
struct OneTuple(usize);

#[derive(Clone, Debug, Dependency)]
struct FiveTuple(usize, i32, i8, f32, String);

#[derive(Clone, Debug, Dependency)]
struct Fields {
    a: usize,
    b: String,
    c: FiveTuple,
}

#[derive(Clone, Debug, Dependency, PartialEq, Eq)]
enum Either<L: Clone + PartialEq + Eq + 'static, R: Clone + PartialEq + Eq + 'static> {
    Left(L),
    Right(R),
}

#[derive(Clone, Dependency)]
struct Result<T: Clone + 'static, E: Clone + 'static>(std::result::Result<T, E>);

// a typemap bind and call should rt
#[test]
fn test_typemap_round_trips() {
    let mut tm = common::TypeMap::new();

    tm.bind(Bare);
    tm.bind(FiveTuple(1, 2, 3, 4., "hello".to_string()));
    tm.bind(Either::<usize, i32>::Right(-1));

    tm.get::<Bare>().unwrap();
    let tup = tm.get::<FiveTuple>().unwrap();
    assert_eq!(tup.0, 1);
    assert_eq!(tup.1, 2);
    assert_eq!(tup.2, 3);
    assert_eq!(tup.3, 4.);
    assert_eq!(tup.4, "hello".to_string());

    let e = tm.get::<Either<usize, i32>>().unwrap();
    assert_eq!(&Either::Right(-1), e);
}
