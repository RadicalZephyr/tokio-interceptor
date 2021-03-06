// This file is part of tokio-interceptor.
//
// tokio-interceptor is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// tokio-interceptor is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with tokio-interceptor.  If not, see <http://www.gnu.org/licenses/>.

use std::mem;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use futures::{future,Future};

use super::{Context,Interceptor};

pub trait Effect {
    fn action(self: Box<Self>);
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
        let effects = mem::replace(&mut context.effects, vec![]);
        for e in effects.into_iter() {
            e.action();
        }
        Box::new(future::ok(context))
    }
}

pub struct MutateState<S, F> {
    state_ref: Option<Rc<RefCell<S>>>,
    mutate: F,
}

impl<S, F> MutateState<S, F> {
    pub fn new(state_ref: Rc<RefCell<S>>, mutate: F) -> MutateState<S, F> {
        MutateState { state_ref: Some(state_ref), mutate }
    }
}

impl<S, F> Effect for MutateState<S, F>
where S: 'static,
      F: 'static + FnOnce(&mut S)
{
    fn action(mut self: Box<Self>) {
        let state_ref = self.state_ref.take().unwrap();
        let mut state = state_ref.borrow_mut();
        (self.mutate)(&mut state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use Context;

    use tests::State;

    #[test]
    fn test_effect_interceptor() {
        let mut context: Context<()> = Context::new(vec![]);
        let i: HandleEffects<()> = HandleEffects::new();

        let state = Rc::new(RefCell::new(State(0)));
        let e = MutateState::new(Rc::clone(&state), |state: &mut State| state.0 = 10);
        context.effects.push(Box::new(e));
        i.after(context);

        assert_eq!(state.borrow().0, 10);
    }
}
