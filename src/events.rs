use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use futures::{future,Future};
use tokio_core::reactor::Handle;

use effects::Effect;
use super::{Coeffect,Context,Dispatched,Interceptor,NewCoeffect};

pub trait Event<E> {
    fn handle(self: Box<Self>, context: Context<E>) -> Box<Future<Item = Context<E>, Error = E>>;
}

pub(crate) struct EventInterceptor<T: Event<E>, E>(RefCell<Option<T>>, PhantomData<E>);

impl<T: Event<E>, E> EventInterceptor<T, E> {
    pub fn new(event: T) -> EventInterceptor<T, E> {
        EventInterceptor(RefCell::new(Some(event)), PhantomData)
    }
}

impl<E: 'static, T: Event<E>> Interceptor for EventInterceptor<T, E> {
    type Error = E;
    fn before(&self, context: Context<Self::Error>) -> Box<Future<Item = Context<Self::Error>,
                                                                  Error = Self::Error>> {
        let mut cell = self.0.borrow_mut();
        let event = cell.take();
        (Box::new(event.unwrap())).handle(context)
    }
}

pub struct Dispatcher<E>{
    handle: Handle,
    dispatcher: Rc<RefCell<EventDispatcher<E>>>,
}

impl<E> Dispatcher<E>
where E: 'static,
{
    pub fn new(handle: &Handle, dispatcher: &Rc<RefCell<EventDispatcher<E>>>) -> Dispatcher<E> {
        Dispatcher { handle: handle.clone(), dispatcher: Rc::clone(dispatcher) }
    }

    pub fn dispatch<Ev>(&self, event: Ev) -> Box<Effect>
    where Ev: 'static + Event<E>
    {
        Box::new(Dispatch::new(event, &self.handle, &self.dispatcher))
    }
}

impl<E> Clone for Dispatcher<E>
where E: 'static,
{
    fn clone(&self) -> Dispatcher<E> {
        Dispatcher::new(&self.handle, &self.dispatcher)
    }
}

impl<E: 'static> Coeffect for Dispatcher<E> {}

impl<E: 'static> NewCoeffect for Dispatcher<E> {
    type Instance = Dispatcher<E>;

    fn new_coeffect(&self) -> Dispatcher<E> {
        self.clone()
    }
}

pub struct Dispatch<E, Err> {
    event: E,
    handle: Handle,
    dispatcher: Rc<RefCell<EventDispatcher<Err>>>,
}

impl<E, Err> Dispatch<E, Err>
where E: 'static + Event<Err>,
      Err: 'static,
{
    pub fn new(event: E, handle: &Handle, dispatcher: &Rc<RefCell<EventDispatcher<Err>>>) -> Dispatch<E, Err> {
        Dispatch {
            event,
            handle: handle.clone(),
            dispatcher: Rc::clone(dispatcher)
        }
    }

    pub fn into_parts(self) -> (E, Handle, Rc<RefCell<EventDispatcher<Err>>>) {
        (self.event, self.handle, self.dispatcher)
    }
}

impl<E, Err> Effect for Dispatch<E, Err>
where E: 'static + Event<Err>,
      Err: 'static,
{
    fn action(self: Box<Self>) {
        let (event, handle, dispatcher) = self.into_parts();
        handle.spawn(dispatcher.borrow().dispatch(event).map(|_| ()).map_err(|_| ()));
    }
}

pub struct EventDispatcher<E> {
    event_handlers: HashMap<TypeId, Vec<Rc<Box<Interceptor<Error = E>>>>>,
}

impl<E: 'static> EventDispatcher<E> {
    pub fn new() -> EventDispatcher<E> {
        EventDispatcher {
            event_handlers: HashMap::new(),
        }
    }

    pub fn register_event<Ev: 'static + Event<E>>(&mut self, interceptors: Vec<Box<Interceptor<Error = E>>>) {
        self.event_handlers.insert(TypeId::of::<Ev>(),
                                   interceptors.into_iter().map(|i| Rc::new(i)).collect());
    }

    pub fn dispatch<Ev: 'static + Event<E>>(&self, event: Ev) -> impl Future<Item = Context<E>> {
        if let Some(interceptors) = self.event_handlers.get(&TypeId::of::<Ev>()) {
            let mut interceptors: Vec<Rc<Box<Interceptor<Error = E>>>> = interceptors.iter().map(Rc::clone).collect();
            interceptors.push(Rc::new(Box::new(EventInterceptor::new(event)) as Box<Interceptor<Error = E>>));
            let mut context = Context::new(interceptors);
            Dispatched::new(Box::new(future::ok(context)))
        } else {
            Dispatched::new(Box::new(future::ok(Context::new(vec![]))))
        }
    }
}
