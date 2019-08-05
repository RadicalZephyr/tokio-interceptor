use std::any::{Any, TypeId};
use std::collections::HashMap;

trait Effector {
    type Effect: Any;

    fn process(&mut self, effect: &Self::Effect);
}

trait EffectorObj {
    fn effect_type_id(&self) -> TypeId;

    fn process(&mut self, effect: Box<dyn Any>);
}

impl<T> EffectorObj for T
where
    T: Effector,
{
    fn effect_type_id(&self) -> TypeId {
        TypeId::of::<<T as Effector>::Effect>()
    }

    fn process(&mut self, effect: Box<dyn Any>) {
        if let Some(effect) = effect.downcast_ref::<<T as Effector>::Effect>() {
            self.process(effect)
        }
    }
}

trait Event<M> {
    fn process(&self, model: &M) -> Vec<Box<dyn Any>>;
}

struct App<Model> {
    model: Model,
    effectors: HashMap<TypeId, Box<dyn EffectorObj>>,
}

impl<M> App<M> {
    pub fn new(model: M, effectors: impl IntoIterator<Item = Box<dyn EffectorObj>>) -> App<M> {
        let effectors = effectors
            .into_iter()
            .map(|e| (e.effect_type_id(), e))
            .collect();
        App { model, effectors }
    }

    fn process_effects(&mut self, effects: Vec<dyn Any>) {}
}

struct Dispatch<M>(Box<dyn Event<M>>);

macro_rules! effector_vec {
    { $( $item:expr ),* } => {
        vec![ $( Box::new({ $item }) as Box<dyn EffectorObj> ),* ]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct FakeEffect;
    struct FakeEffector;
    impl Effector for FakeEffector {
        type Effect = FakeEffect;

        fn process(&mut self, effect: &FakeEffect) {}
    }

    #[test]
    fn test_build() {
        let app = App::new((), effector_vec![FakeEffector]);
    }
}
