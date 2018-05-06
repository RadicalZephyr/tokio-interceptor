extern crate anymap;
#[macro_use]
extern crate futures;

use std::any::TypeId;
use std::collections::{HashMap,VecDeque};
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

pub trait Event<E> {
    fn handle(&self, context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>>;
}

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

/// Dispatched represents the eventual completion of an Event
/// being fully processed by a chain of Interceptors.  First, the
/// chain is iterated in order threading the Context through each
/// `before` method. On reaching the end of the chain, the
/// interceptors are iterated in the reverse order, and the
/// Context is threaded through their `after` methods.
enum Dispatched<E> {
    Forwards(Box<Future<Item = Context<E>, Error = E>>),
    Backwards(Box<Future<Item = Context<E>, Error = E>>),
    Done(Context<E>),
    Empty,
}

impl<E: 'static> Future for Dispatched<E> {
    type Item = Context<E>;
    type Error = E;

    fn poll(&mut self) -> Result<Async<Context<E>>, E> {
        match *self {
            Dispatched::Empty => (),
            Dispatched::Done(ref _ctx) => (),
            Dispatched::Forwards(ref mut future_ctx) => {
                let mut ctx = try_ready!(future_ctx.poll());
                if let Some(next) = ctx.queue.pop_front() {
                    next.before(ctx);

                } else {

                }
            },
            Dispatched::Backwards(ref mut future_ctx) => (),
        };
        Ok(Async::Ready(Context::new(vec![])))
    }
}

struct EventDispatcher<E> {
    event_handlers: HashMap<TypeId, Vec<Rc<Box<Interceptor<Error = E>>>>>,
}

impl EventDispatcher<()> {
    pub fn new() -> EventDispatcher<()> {
        EventDispatcher {
            event_handlers: HashMap::new(),
        }
    }

    pub fn register_event<Ev: 'static + Event<()>>(&mut self, interceptors: Vec<Box<Interceptor<Error = ()>>>) {
        self.event_handlers.insert(TypeId::of::<Ev>(),
                                   interceptors.into_iter().map(|i| Rc::new(i)).collect());
    }

    pub fn dispatch<Ev: 'static + Event<()>>(&self, event: Ev) -> Dispatched<()> {
        if let Some(interceptors) = self.event_handlers.get(&TypeId::of::<Ev>()) {
            let mut interceptors: Vec<Rc<Box<Interceptor<Error = ()>>>> = interceptors.iter().map(|i| Rc::clone(i)).collect();
            interceptors.push(Rc::new(Box::new(event) as Box<Interceptor<Error = ()>>));
            let mut context = Context::new(interceptors);
            Dispatched::Forwards(Box::new(future::ok(context)))
        } else {
            Dispatched::Empty
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
        fn handle(&self, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
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
        fn handle(&self, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
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
}
