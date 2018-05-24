extern crate anymap;
#[macro_use]
extern crate futures;

use std::collections::VecDeque;
use std::mem;
use std::sync::Arc;
use std::rc::Rc;

use anymap::AnyMap;
use futures::{future,Async,Future};

mod coeffects;
pub use coeffects::{Coeffect,NewCoeffect,InjectCoeffect};

mod db;
pub use db::Db;

mod effects;
pub use effects::{Effect,HandleEffects};

mod events;
pub use events::{Event,EventDispatcher,Dispatch,Dispatcher};

pub struct Context<E> {
    pub coeffects: AnyMap,
    pub effects: Vec<Box<Effect>>,
    pub queue: VecDeque<Rc<Box<Interceptor<Error = E>>>>,
    pub stack: Vec<Rc<Box<Interceptor<Error = E>>>>,
}

impl<E> Context<E> {
    pub fn new(interceptors: Vec<Rc<Box<Interceptor<Error = E>>>>) -> Context<E> {
        Context {
            coeffects: AnyMap::new(),
            effects: vec![],
            queue: interceptors.into_iter().collect(),
            stack: vec![],
        }
    }

    pub fn push_effect<Eff: 'static + Effect>(&mut self, effect: Eff) {
        self.effects.push(Box::new(effect));
    }

    pub fn next(self) -> Box<Future<Item = Context<E>, Error = E>>
    where E: 'static
    {
        Box::new(future::ok(self))
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

enum Direction {
    Forwards, Backwards
}

impl Direction {
    fn call<E>(&self, interceptor: Rc<Box<Interceptor<Error = E>>>, context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>>
    where E: 'static
    {
        match *self {
            Direction::Forwards => interceptor.before(context),
            Direction::Backwards => interceptor.after(context),
        }
    }

    fn is_forwards(&self) -> bool {
        match *self {
            Direction::Forwards => true,
            Direction::Backwards => false,
        }
    }

    fn is_backwards(&self) -> bool {
        match *self {
            Direction::Forwards => false,
            Direction::Backwards => true,
        }
    }
}

/// Dispatched represents the eventual completion of an Event
/// being fully processed by a chain of Interceptors.  First, the
/// chain is iterated in order threading the Context through each
/// `before` method. On reaching the end of the chain, the
/// interceptors are iterated in the reverse order, and the
/// Context is threaded through their `after` methods.
struct Dispatched<E> {
    direction: Direction,
    next_ctx: Box<Future<Item = Context<E>, Error = E>>,
}

impl<E> Dispatched<E> {
    pub fn new(next_ctx: Box<Future<Item = Context<E>, Error = E>>) -> Dispatched<E> {
        Dispatched {
            direction: Direction::Forwards,
            next_ctx,
        }
    }
}

impl<E: 'static> Future for Dispatched<E> {
    type Item = Context<E>;
    type Error = E;

    fn poll(&mut self) -> Result<Async<Context<E>>, E> {
        loop {
            let mut ctx = try_ready!(self.next_ctx.poll());
            if let Some(next) = ctx.queue.pop_front() {
                ctx.stack.push(Rc::clone(&next));
                self.next_ctx = self.direction.call(next, ctx);
                continue;
            } else {
                if self.direction.is_forwards() {
                    self.direction = Direction::Backwards;
                    let stack = mem::replace(&mut ctx.stack, vec![]);
                    ctx.queue = stack.into_iter().rev().collect();
                    self.next_ctx = Box::new(future::ok(ctx));
                    continue;
                } else {
                    return Ok(Async::Ready(ctx));
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use std::cell::RefCell;
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

    struct BeforeEvent(pub Rc<RefCell<bool>>);

    impl Event<()> for BeforeEvent {
        fn handle(self: Box<Self>, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
            let mut called = self.0.borrow_mut();
            *called = true;
            Box::new(future::ok(context))
        }
    }

    #[test]
    fn test_dispatcher_calls_event_before() {
        let mut app = EventDispatcher::new();
        app.register_event::<BeforeEvent>(vec![]);
        let called = Rc::new(RefCell::new(false));
        app.dispatch(BeforeEvent(Rc::clone(&called))).wait();
        assert_eq!(true, *called.borrow());
    }

    struct BeforeInter(pub Rc<RefCell<bool>>);

    impl Interceptor for BeforeInter {
        type Error = ();

        fn before(&self, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
            let mut called = self.0.borrow_mut();
            *called = true;
            Box::new(future::ok(context))
        }
    }

    struct IdentityEvent;
    impl Event<()> for IdentityEvent {
        fn handle(self: Box<Self>, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
            Box::new(future::ok(context))
        }
    }

    #[test]
    fn test_dispatcher_calls_interceptor_before() {
        let mut app = EventDispatcher::new();

        let called_first = Rc::new(RefCell::new(false));
        let before_inter = BeforeInter(Rc::clone(&called_first));
        app.register_event::<BeforeEvent>(vec![Box::new(before_inter)]);

        let called_second = Rc::new(RefCell::new(false));
        app.dispatch(BeforeEvent(Rc::clone(&called_second))).wait();

        assert_eq!(true, *called_first.borrow());
        assert_eq!(true, *called_second.borrow());
    }

    struct AfterInter(pub Rc<RefCell<bool>>);

    impl Interceptor for AfterInter {
        type Error = ();

        fn after(&self, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
            let mut called = self.0.borrow_mut();
            *called = true;
            Box::new(future::ok(context))
        }
    }

    #[test]
    fn test_dispatcher_calls_interceptor_after() {
        let mut app = EventDispatcher::new();

        let called_first = Rc::new(RefCell::new(false));
        let before_inter = BeforeInter(Rc::clone(&called_first));

        let called_third = Rc::new(RefCell::new(false));
        let after_inter = AfterInter(Rc::clone(&called_third));

        app.register_event::<BeforeEvent>(vec![Box::new(before_inter),
                                               Box::new(after_inter)]);

        let called_second = Rc::new(RefCell::new(false));
        app.dispatch(BeforeEvent(Rc::clone(&called_second))).wait();

        assert_eq!(true, *called_first.borrow());
        assert_eq!(true, *called_second.borrow());
        assert_eq!(true, *called_third.borrow());
    }
}
