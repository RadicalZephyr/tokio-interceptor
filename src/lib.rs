extern crate anymap;
extern crate futures;

use std::collections::VecDeque;
use std::sync::Arc;
use std::rc::Rc;

use anymap::AnyMap;
use futures::{future,Future};

mod coeffects;
pub use coeffects::{Coeffect,NewCoeffect,InjectCoeffect};

mod db;
pub use db::Db;

mod effects;
pub use effects::{Effect,HandleEffects};

pub trait Event<E> {
    fn handle(&self, context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>>;
}

pub struct Context<E> {
    pub coeffects: AnyMap,
    pub effects: Vec<Box<Effect>>,
    pub queue: VecDeque<Box<Interceptor<Error = E>>>,
    pub stack: Vec<Box<Interceptor<Error = E>>>,
}

impl<E> Context<E> {
    pub fn new() -> Context<E> {
        Context {
            coeffects: AnyMap::new(),
            effects: vec![],
            queue: VecDeque::new(),
            stack: vec![],
        }
    }
}

pub trait Interceptor {
    type Error: 'static;

    fn before(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                  Error = Self::Error>> {
        Box::new(future::ok(context))
    }

    fn after(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                 Error = Self::Error>> {
        Box::new(future::ok(context))
    }
}

impl<T: Event<()>> Interceptor for T {
    type Error = ();
    fn before(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                  Error = Self::Error>> {
        (self).handle(context)
    }
}

impl<I: Interceptor + ?Sized> Interceptor for Arc<I> {
    type Error = I::Error;

    fn before(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                  Error = Self::Error>> {
        (**self).before(context)
    }

    fn after(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                 Error = Self::Error>> {
        (**self).after(context)
    }
}

impl<I: Interceptor + ?Sized> Interceptor for Rc<I> {
    type Error = I::Error;

    fn before(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                  Error = Self::Error>> {
        (**self).before(context)
    }

    fn after(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                 Error = Self::Error>> {
        (**self).after(context)
    }
}

pub trait NewInterceptor
{
    type Error: 'static;
    type Interceptor: Interceptor<Error = Self::Error>;

    fn new_interceptor(&self) -> Self::Interceptor;
}


impl<I: Copy + Interceptor> NewInterceptor for I {
    type Error = I::Error;
    type Interceptor = I;

    fn new_interceptor(&self) -> I {
        *self
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use std::rc::Rc;

    #[derive(Debug,PartialEq)]
    pub struct State(pub u8);

    pub struct StateHolder(pub Rc<State>);

    impl NewCoeffect for StateHolder {
        type Instance = Rc<State>;

        fn new_coeffect(&self) -> Rc<State> {
            Rc::clone(&self.0)
        }
    }

    impl Coeffect for State {}

    #[test]
    fn test_coeffect_map() {
        let mut cmap = AnyMap::new();
        cmap.insert(State(1));
        assert_eq!(Some(&State(1)), cmap.get::<State>())
    }
}
