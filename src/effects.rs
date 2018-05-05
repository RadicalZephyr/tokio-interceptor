use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use futures::{future,Future};

use super::{Context,Interceptor};

pub trait Effect {
    fn action(&mut self);
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

#[cfg(test)]
mod tests {
    use super::*;

    use Context;

    use tests::State;

    #[test]
    fn test_effect_interceptor() {
        let mut context: Context<()> = Context::new(&vec![]);
        let i: HandleEffects<()> = HandleEffects::new();

        let state = Rc::new(RefCell::new(State(0)));
        let e = MutateState::new(Rc::clone(&state), |state: &mut State| state.0 = 10);
        context.effects.push(Box::new(e));
        i.after(context);

        assert_eq!(state.borrow().0, 10);
    }
}
