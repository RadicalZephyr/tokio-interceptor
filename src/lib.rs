use std::any::{Any, TypeId};
use std::collections::HashMap;

struct Handler {

}

trait Event {}

struct Registry {
    events: HashMap<TypeId, Handler>,
}

impl Registry {
    fn new() -> Registry {
        Registry { events: HashMap::new() }
    }
}

impl Registry {
    fn register_event<T: ?Sized + Any>(&mut self, handler: Handler) {
        self.events.insert(TypeId::of::<T>(), handler);
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
