use std::collections::VecDeque;
use std::iter::{FromIterator,IntoIterator,Iterator};
use std::rc::Rc;

use super::Interceptor;

pub struct InterceptorQueue<E>(VecDeque<Rc<Box<Interceptor<Error = E>>>>);

impl<E> InterceptorQueue<E> {
    pub fn push_back(&mut self, value: Rc<Box<Interceptor<Error = E>>>) {
        self.0.push_back(value);
    }

    pub fn pop_front(&mut self) -> Option<Rc<Box<Interceptor<Error = E>>>> {
        self.0.pop_front()
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }
}

impl<E> FromIterator<Rc<Box<Interceptor<Error = E>>>> for InterceptorQueue<E> {
    fn from_iter<T>(iter: T) -> InterceptorQueue<E>
    where T: IntoIterator<Item = Rc<Box<Interceptor<Error = E>>>>
    {
        InterceptorQueue(iter.into_iter().collect())
    }
}

impl<E> Extend<Box<Interceptor<Error = E>>> for InterceptorQueue<E> {
    fn extend<T>(&mut self, iter: T)
    where T: IntoIterator<Item = Box<Interceptor<Error = E>>>
    {
        self.0.extend(iter.into_iter().map(|i| Rc::new(i)))
    }
}
