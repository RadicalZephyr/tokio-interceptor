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

use std::cell::{Ref,RefCell};
use std::rc::Rc;

use {Coeffect,NewCoeffect};
use effects::MutateState;

pub struct Db<State>(Rc<RefCell<State>>);

impl<State> Db<State>
where State: Clone,
{
    pub fn new(state: State) -> Db<State> {
        Db(Rc::new(RefCell::new(state)))
    }

    pub fn borrow(&self) -> Ref<State> {
        self.0.borrow()
    }

    pub fn mutate<F>(&self, f: F) -> MutateState<State, F> {
        MutateState::new(Rc::clone(&self.0), f)
    }

    pub fn update(&self) -> State {
        self.0.borrow().clone()
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

    use {Context,Event,Interceptor,InjectCoeffect,HandleEffects};
    use events::EventInterceptor;
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
        fn handle(self: Box<Self>, mut context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>> {
            {
                let db = context.coeffects.get::<Db<State>>().unwrap();
                assert_eq!(self.initial, db.borrow().0);
                let inc = self.inc;
                let new_state = db.update();
                new_state.0 += inc;
                context.effects.push(Box::new(new_state));
            }
            Box::new(future::ok(context))
        }
    }

    #[test]
    fn test_event_as_interceptor() {
        let mut context: Context<()> = Context::new(vec![]);
        let event = Plus::new(101, 10);
        let db = Db::new(State(101));
        let i_state = InjectCoeffect::<Db<State>, ()>::new(db.clone());
        let i_effects: HandleEffects<()> = HandleEffects::new();
        let i_event = EventInterceptor::new(event);

        let queue = vec![Box::new(i_state) as Box<Interceptor<Error = ()>>,
                         Box::new(i_effects) as Box<Interceptor<Error = ()>>,
                         Box::new(i_event) as Box<Interceptor<Error = ()>>];
        let mut stack = vec![];
        for i in queue.into_iter() {
            context = i.before(context).wait().unwrap();
            stack.push(i);
        }
        for i in stack.into_iter() {
            context = i.after(context).wait().unwrap();
        }

        assert_eq!(State(111), *db.borrow());
    }
}
