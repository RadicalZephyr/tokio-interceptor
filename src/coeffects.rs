use std::any::Any;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use futures::{future,Future};

use super::{Context,Interceptor};

pub trait Coeffect: Any {}

impl<C: Coeffect + ?Sized> Coeffect for Arc<C> {}
impl<C: Coeffect + ?Sized> Coeffect for Rc<C> {}
impl<C: Coeffect + ?Sized> Coeffect for Box<C> {}

pub trait NewCoeffect {
    type Instance: Coeffect;

    fn new_coeffect(&self) -> Self::Instance;
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::rc::Rc;

    use tests::{State,StateHolder};

    #[test]
    fn test_coeffect_interceptor() {
        let context: Context<()> = Context::new();
        let state_holder = StateHolder(Rc::new(State(101)));
        let i = InjectCoeffect::<StateHolder, ()>::new(state_holder);
        let new_ctx = i.before(context).wait().unwrap();
        assert_eq!(State(101), **new_ctx.coeffects.get::<Rc<State>>().unwrap());
    }
}
