use std::cell::{Ref,RefCell};
use std::rc::Rc;

use {Coeffect,NewCoeffect};
use effects::MutateState;

pub struct Db<State>(Rc<RefCell<State>>);

impl<State> Db<State> {
    pub fn new(state: State) -> Db<State> {
        Db(Rc::new(RefCell::new(state)))
    }

    pub fn borrow(&self) -> Ref<State> {
        self.0.borrow()
    }

    pub fn mutate<F>(&self, f: F) -> MutateState<State, F> {
        MutateState::new(Rc::clone(&self.0), f)
    }
}

impl<S> Clone for Db<S> {
    fn clone(&self) -> Db<S> {
        Db(Rc::clone(&self.0))
    }
}

impl<S: 'static> Coeffect for Db<S> {}

impl<S: 'static> NewCoeffect for Db<S> {
    type Instance = Db<S>;

    fn new_coeffect(&self) -> Db<S> {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use {Context,Event,Interceptor,InjectCoeffect,HandleEffects,EventInterceptor};
    use tests::State;
    use futures::{future,Future};

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
                let db = context.coeffects.get::<Db<State>>().unwrap();
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
    fn test_event_as_interceptor() {
        let context: Context<()> = Context::new();
        let event = Plus::new(101, 10);
        let db = Db::new(State(101));
        let i_state = InjectCoeffect::<Db<State>, ()>::new(db.clone());
        let i_effects: HandleEffects<()> = HandleEffects::new();
        let i_event: EventInterceptor<Plus, ()> = EventInterceptor::new(event);

        let ctx1 = i_state.before(context).wait().unwrap();
        let ctx2 = i_event.before(ctx1).wait().unwrap();
        let _after_ctx = i_effects.after(ctx2).wait().unwrap();

        assert_eq!(State(111), *db.borrow());
    }
}
