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

use std::collections::VecDeque;
use std::iter::{FromIterator,IntoIterator,Iterator};
use std::rc::Rc;

use super::Interceptor;

pub struct InterceptorQueue<E>(VecDeque<Rc<Box<Interceptor<Error = E>>>>);

impl<E> InterceptorQueue<E> {
    pub fn push_back<T>(&mut self, value: T)
    where T: Into<Rc<Box<Interceptor<Error = E>>>>,
    {
        self.0.push_back(value.into());
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
