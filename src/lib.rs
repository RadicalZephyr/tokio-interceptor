#![feature(get_type_id)]

#[macro_use]
extern crate mopa;

use std::any::TypeId;
use std::collections::HashMap;

use mopa::Any;

struct Handler {

}

trait Event: Any {}

mopafy!(Event);

struct Registry {
    events: HashMap<TypeId, Handler>,
}

impl Registry {
    fn new() -> Registry {
        Registry { events: HashMap::new() }
    }
}

struct Context {

}

impl Context {
    fn new(event: Box<Event>) -> Context {
        Context {  }
    }
}

impl Registry {
    fn register_event<T: ?Sized + Any>(&mut self, handler: Handler) {
        self.events.insert(TypeId::of::<T>(), handler);
    }

    fn call(&self, event: Box<Event>) {
        let handler = self.events.get(&event.get_type_id());
        let context = Context::new(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FooEvent(u8);

    impl Event for FooEvent {}

    #[test]
    fn it_works() {
        let mut r = Registry::new();
        r.register_event::<FooEvent>(Handler {});
    }
}
