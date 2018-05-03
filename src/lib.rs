extern crate anymap;
extern crate futures;

use std::collections::VecDeque;
use std::marker::PhantomData;

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

pub struct EventInterceptor<T, E>(T, PhantomData<E>);

impl<T, E> EventInterceptor<T, E> {
    pub fn new(event: T) -> EventInterceptor<T, E> {
        EventInterceptor(event, PhantomData)
    }
}

impl<T, E> Interceptor for EventInterceptor<T, E>
where T: Event<E>,
      E: 'static,
{
    type Error = E;

    fn before(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                      Error = Self::Error>> {
        self.0.handle(context)
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
