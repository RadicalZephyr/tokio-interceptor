extern crate anymap;
extern crate futures;

use std::collections::VecDeque;
use std::marker::PhantomData;

use anymap::AnyMap;
use futures::{future,Future};

mod coeffects;
pub use coeffects::{Coeffect,NewCoeffect,InjectCoeffect};

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

    use std::cell::{Ref, RefCell};
    use std::rc::Rc;

    use effects::MutateState;

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

    struct Db(Rc<RefCell<State>>);

    impl Db {
        pub fn new(state: State) -> Db {
            Db(Rc::new(RefCell::new(state)))
        }

        pub fn borrow(&self) -> Ref<State> {
            self.0.borrow()
        }

        pub fn mutate<F>(&self, f: F) -> MutateState<State, F> {
            MutateState::new(Rc::clone(&self.0), f)
        }
    }

    impl Clone for Db {
        fn clone(&self) -> Db {
            Db(Rc::clone(&self.0))
        }
    }

    impl Coeffect for Db {}

    impl NewCoeffect for Db {
        type Instance = Db;

        fn new_coeffect(&self) -> Db {
            self.clone()
        }
    }

    #[derive(Debug,Default,PartialEq)]
    struct Plus{
        initial: u8,
        inc: u8,
    }

    impl Plus {
        pub fn new(initial: u8, inc: u8) -> Plus {
            Plus { initial, inc }
        }
    }

    impl<E> Event<E> for Plus
    where E: 'static,
    {
        fn handle(&self, mut context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>> {
            {
                let db = context.coeffects.get::<Db>().unwrap();
                assert_eq!(self.initial, db.borrow().0);
                let inc = self.inc;
                context.effects.push(Box::new(db.mutate(move |state: &mut State| {
                    state.0 += inc;
                })));
            }
            Box::new(future::ok(context))
        }
    }

    #[test]
    fn test_event_as_coeffect() {
        let mut context: Context<()> = Context::new();
        let event = Plus::new(0, 10);
        context.coeffects.insert(event);
        assert_eq!(Some(&Plus::new(0, 10)), context.coeffects.get::<Plus>())
    }

    #[test]
    fn test_event_as_interceptor() {
        let context: Context<()> = Context::new();
        let event = Plus::new(101, 10);
        let db = Db::new(State(101));
        let i_state = InjectCoeffect::<Db, ()>::new(db.clone());
        let i_effects: HandleEffects<()> = HandleEffects::new();
        let i_event: EventInterceptor<Plus, ()> = EventInterceptor::new(event);

        let ctx1 = i_state.before(context).wait().unwrap();
        let ctx2 = i_event.before(ctx1).wait().unwrap();
        let _after_ctx = i_effects.after(ctx2).wait().unwrap();

        assert_eq!(State(111), *db.borrow());
    }
}
