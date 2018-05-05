extern crate anymap;
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
            let next_state = match mem::replace(self, Dispatched::Empty) {
                Dispatched::Empty => return Ok(Async::Ready(Context::new(vec![]))),
                Dispatched::Done(context) => return Ok(Async::Ready(context)),
                Dispatched::Forwards(ref mut ctx) => match ctx.poll() {
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(e) => return Err(e),
                    Ok(Async::Ready(mut ctx)) => {
                        if let Some(next_interceptor) = ctx.queue.pop_front() {
                            ctx.stack.push(Rc::clone(&next_interceptor));
                            Dispatched::Forwards(next_interceptor.before(ctx))
                        } else {
                            let stack = mem::replace(&mut ctx.stack, vec![]);
                            ctx.queue = stack.into_iter().collect();
                            Dispatched::Backwards(Box::new(future::ok(ctx)))
                        }
                    }
                },
                Dispatched::Backwards(ref mut ctx) => match ctx.poll() {
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(e) => return Err(e),
                    Ok(Async::Ready(mut ctx)) => {
                        if let Some(next_interceptor) = ctx.queue.pop_front() {
                            Dispatched::Forwards(next_interceptor.after(ctx))
                        } else {
                            Dispatched::Done(ctx)
                        }
                    }
                }
            };
            *self = next_state;
            Ok(Async::NotReady)
        }
    }

    struct EventDispatcher<E> {
        event_handlers: HashMap<TypeId, Vec<Rc<Box<Interceptor<Error = E>>>>>,
    }

    impl<Err: 'static> EventDispatcher<Err> {
        pub fn new() -> EventDispatcher<Err> {
            EventDispatcher {
                event_handlers: HashMap::new(),
            }
        }

        pub fn register_event<Ev: 'static + Event<Err>>(&mut self, interceptors: Vec<Box<Interceptor<Error = Err>>>) {
            self.event_handlers.insert(TypeId::of::<Ev>(),
                                       interceptors.into_iter().map(|i| Rc::new(i)).collect());
        }

        pub fn dispatch<Ev: 'static + Event<Err>>(&self, event: Ev) -> Dispatched<Err> {
            if let Some(interceptors) = self.event_handlers.get(&TypeId::of::<Ev>()) {
                let interceptors = interceptors.iter().map(|i| Rc::clone(i)).collect();
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

    struct FooEvent;

    impl Event<()> for FooEvent {
        fn handle(&self, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
            Box::new(future::ok(context))
        }
    }

    #[test]
    fn test_dispatcher_registers_events() {
        let mut app = EventDispatcher::new();
        app.register_event::<FooEvent>(vec![]);
        app.dispatch(FooEvent{});
    }
}
