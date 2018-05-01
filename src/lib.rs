#![feature(get_type_id)]

#[macro_use]
extern crate mopa;

use std::any::TypeId;
use std::collections::HashMap;

use mopa::Any;

macro_rules! with_any_map {
    ($trait_:ident, $mapname:ident) => {
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
        }
    }
}

pub trait Coeffect: Any {}
mopafy!(Coeffect);
with_any_map!(Coeffect, CoeffectMap);

pub trait Effect: Any {}
mopafy!(Effect);
with_any_map!(Effect, EffectMap);

pub trait Event: Any {
    fn handle(&self);
}
mopafy!(Event);
with_any_map!(Event, EventMap);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug,PartialEq)]
    struct Test(u8);

    impl Coeffect for Test {

    }

    #[test]
    fn test_coeffect_map() {
        let mut cmap = CoeffectMap::new();
        cmap.insert(Test(1));
        assert_eq!(Some(&Test(1)), cmap.get::<Test>())
    }
}
