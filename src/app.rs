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

use std::cell::RefCell;
use std::rc::Rc;

use futures::Future;
use tokio_core::reactor::Handle;

use super::{Db, Dispatcher, Event, EventDispatcher,
            HandleEffects, InjectCoeffect, Interceptor};

pub struct App<State> {
    handle: Handle,
    db: Db<State>,
    dispatcher: Rc<RefCell<EventDispatcher<()>>>,
}

impl<State> App<State>
where State: 'static + Default,
{
    pub fn new(handle: Handle) -> App<State> {
        App { handle,
              db: Db::new(State::default()),
              dispatcher: Rc::new(RefCell::new(EventDispatcher::new())) }
    }

    pub fn default_interceptors(&self) -> Vec<Box<Interceptor<Error = ()>>> {
        let inject_state = InjectCoeffect::<Db<State>, ()>::new(self.db.clone());
        let inject_dispatcher = InjectCoeffect::<Dispatcher<()>, ()>::new(Dispatcher::new(&self.handle, &self.dispatcher));
        let handle_effects = HandleEffects::new();
        vec![Box::new(inject_state), Box::new(inject_dispatcher), Box::new(handle_effects)]
    }

    pub fn register_event<E: 'static + Event<()>>(&mut self) {
        self.register_event_with::<E>(vec![]);
    }

    pub fn register_event_with<E: 'static + Event<()>>(&mut self, mut interceptors: Vec<Box<Interceptor<Error = ()>>>) {
        let mut i = self.default_interceptors();
        i.append(&mut interceptors);

        match self.dispatcher.try_borrow_mut() {
            Ok(mut dispatcher) => dispatcher.register_event::<E>(i),
            Err(e) => {
                warn!("failed to register event: did not have unique access to EventDispatcher: {}", e);
            },
        };
    }

    pub fn dispatch<E: 'static + Event<()>>(&mut self, e: E) -> impl Future {
        self.dispatcher.borrow().dispatch(e)
    }
}
