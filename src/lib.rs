#[macro_use]
extern crate mopa;
extern crate futures;

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{HashMap,VecDeque};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use futures::{future,Future};

use mopa::Any;

macro_rules! with_any_map {
    ($trait_:ident, $mapname:ident) => {
        #[derive(Default)]
        pub struct $mapname(HashMap<TypeId, Box<$trait_>>);

        impl $mapname {
            pub fn new() -> $mapname {
                $mapname(HashMap::new())
            }

            pub fn get<'a, T: 'static + $trait_>(&'a self) -> Option<&'a T> {
                self.0.get(&TypeId::of::<T>()).and_then(|any| any.downcast_ref::<T>())
            }

            pub fn insert<T: 'static + $trait_>(&mut self, value: T) {
                self.0.insert(TypeId::of::<T>(), Box::new(value));
            }

            pub fn values(&self) -> std::collections::hash_map::Values<TypeId, Box<$trait_>> {
                self.0.values()
            }

            pub fn values_mut(&mut self) -> std::collections::hash_map::ValuesMut<TypeId, Box<$trait_>> {
                self.0.values_mut()
            }
        }
    }
}

pub trait Coeffect: Any {}

mopafy!(Coeffect);
with_any_map!(Coeffect, CoeffectMap);

impl<C: Coeffect + ?Sized> Coeffect for Arc<C> {}
impl<C: Coeffect + ?Sized> Coeffect for Rc<C> {}
impl<C: Coeffect + ?Sized> Coeffect for Box<C> {}


pub trait NewCoeffect {
    type Instance: Coeffect;

    fn new_coeffect(&self) -> Self::Instance;
}

pub trait Effect {
    fn action(&mut self);
}

pub trait Event<E> {
    fn handle(&self, context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>>;
}

#[derive(Default)]
pub struct Context<E> {
    pub coeffects: CoeffectMap,
    pub effects: Vec<Box<Effect>>,
    pub queue: VecDeque<Box<Interceptor<Error = E>>>,
    pub stack: Vec<Box<Interceptor<Error = E>>>,
}

impl<E> Context<E> {
    pub fn new() -> Context<E> {
        Context {
            coeffects: CoeffectMap::new(),
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

#[derive(Default)]
pub struct InjectCoeffect<C, E>(C, PhantomData<E>);

impl<C, E> InjectCoeffect<C, E>
{
    pub fn new(new_coeffect: C) -> InjectCoeffect<C, E> {
        InjectCoeffect(new_coeffect, PhantomData)
    }
}

impl<C, E> Interceptor for InjectCoeffect<C, E>
where C: NewCoeffect,
      E: 'static,
{
    type Error = E;

    fn before(&self, mut context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                  Error = Self::Error>> {
        context.coeffects.insert(self.0.new_coeffect());
        Box::new(future::ok(context))
    }
}

pub struct HandleEffects<E>(PhantomData<E>);

impl<E> HandleEffects<E>
{
    pub fn new() -> HandleEffects<E> {
        HandleEffects(PhantomData)
    }
}

impl<E> Interceptor for HandleEffects<E>
where E: 'static,
{
    type Error = E;

    fn after(&self, mut context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                 Error = Self::Error>> {
        for e in context.effects.iter_mut() {
            e.action();
        }
        Box::new(future::ok(context))
    }
}

pub struct MutateState<S, F> {
    state_ref: Rc<RefCell<S>>,
    mutate: F,
}

impl<S, F> MutateState<S, F> {
    pub fn new(state_ref: Rc<RefCell<S>>, mutate: F) -> MutateState<S, F> {
        MutateState { state_ref, mutate }
    }
}

impl<S, F> Effect for MutateState<S, F>
where S: 'static,
      F: 'static + FnMut(&mut S)
{
    fn action(&mut self) {
        (&mut self.mutate)(&mut self.state_ref.borrow_mut())
    }
}

#[derive(Debug,Default,PartialEq)]
struct EventCoeffect<T, E>(T, PhantomData<E>)
where T: Default, E: Default;

impl<T, E> EventCoeffect<T, E>
where T: Default,
      E: Default
{
    pub fn new(val: T) -> EventCoeffect<T, E> {
        EventCoeffect(val, PhantomData)
    }
}

impl<T, E> Coeffect for EventCoeffect<T, E>
where T: 'static + Event<E> + Default,
      E: 'static + Default,
{}

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
mod tests {
    use super::*;

    use std::cell::{Ref, RefCell};
    use std::rc::Rc;

    #[derive(Debug,PartialEq)]
    struct State(u8);

    struct StateHolder(Rc<State>);

    impl NewCoeffect for StateHolder {
        type Instance = Rc<State>;

        fn new_coeffect(&self) -> Rc<State> {
            Rc::clone(&self.0)
        }
    }

    #[derive(Debug,PartialEq)]
    struct Test(u8);

    impl Coeffect for State {}

    #[test]
    fn test_coeffect_map() {
        let mut cmap = CoeffectMap::new();
        cmap.insert(State(1));
        assert_eq!(Some(&State(1)), cmap.get::<State>())
    }

    #[test]
    fn test_coeffect_interceptor() {
        let context: Context<()> = Context::new();
        let state_holder = StateHolder(Rc::new(State(101)));
        let i = InjectCoeffect::<StateHolder, ()>::new(state_holder);
        let new_ctx = i.before(context).wait().unwrap();
        assert_eq!(State(101), **new_ctx.coeffects.get::<Rc<State>>().unwrap());
    }

    #[test]
    fn test_effect_interceptor() {
        let mut context: Context<()> = Context::new();
        let i: HandleEffects<()> = HandleEffects::new();

        let state = Rc::new(RefCell::new(State(0)));
        let e = MutateState::new(Rc::clone(&state), |state: &mut State| state.0 = 10);
        context.effects.push(Box::new(e));
        i.after(context);

        assert_eq!(state.borrow().0, 10);
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
        let event: EventCoeffect<Plus, ()> = EventCoeffect::new(Plus::new(0, 10));
        context.coeffects.insert(event);
        assert_eq!(Some(&EventCoeffect::new(Plus::new(0, 10))), context.coeffects.get::<EventCoeffect<Plus, ()>>())
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
