extern crate futures;
extern crate tokio_core;
extern crate tokio_interceptor;


use std::{io, thread};
use std::cell::RefCell;
use std::io::BufRead;
use std::rc::Rc;

use futures::stream::iter_result;
use futures::{Future, Sink, Stream};
use futures::sync::mpsc::{unbounded, SendError, UnboundedReceiver};
use tokio_core::reactor::{Core, Handle};
use tokio_interceptor::{Context, Db, Dispatcher, Effect,
                        Event, EventDispatcher, EventInterceptor, HandleEffects,
                        InjectCoeffect, Interceptor};

struct App<State> {
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

    fn default_interceptors(&self) -> Vec<Box<Interceptor<Error = ()>>> {
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
                println!("failed to register event: did not have unique access to EventDispatcher: {}", e);
            },
        };
    }

    pub fn dispatch<E: 'static + Event<()>>(&mut self, e: E) {
        self.handle.spawn(self.dispatcher.borrow().dispatch(e).map(|_| ()).map_err(|_| ()));
    }
}

#[derive(Debug)]
enum Error {
    Stdin(std::io::Error),
    Channel(SendError<String>),
}

/// Spawn a new thread that reads from stdin and passes messages back using an unbounded channel.
pub fn spawn_stdin_stream_unbounded() -> UnboundedReceiver<String> {
    let (channel_sink, channel_stream) = unbounded();
    let stdin_sink = channel_sink.sink_map_err(Error::Channel);

    thread::spawn(move || {
        let stdin = io::stdin();
        let stdin_lock = stdin.lock();
        iter_result(stdin_lock.lines())
            .map_err(Error::Stdin)
            .forward(stdin_sink)
            .wait()
            .unwrap();
    });

    channel_stream
}

enum Mode {
    Adding, Removing, Marking, Menu
}

struct AppState {
    mode: Mode,
    todos: Vec<String>,
}

impl Default for AppState {
    fn default() -> AppState {
        AppState {
            mode: Mode::Menu,
            todos: vec![],
        }
    }
}

struct Print(String);

impl Effect for Print {
    fn action(self: Box<Self>) {
        println!("{}", self.0);
    }
}

struct ShowMenu;

impl Event<()> for ShowMenu {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        let menu = format!(r#"
---
What do you want to do?
1 - Display tasks
2 - Add a new task
3 - Mark a task as done
4 - Remove a task
---
"#);
        context.push_effect(Print(menu));
        context.next()
    }
}

struct Input(pub String);

impl Event<()> for Input {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            let input = match self.0.trim() {
                "" => None,
                input => Some(input.to_string()),
            };
            if let Some(input) = input {
                match db.borrow().mode {
                    Mode::Menu     => {
                        let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                            Box::new(EventInterceptor::new(MenuInput))
                        ];
                        context.queue.extend(interceptors);
                    },
                    Mode::Adding   => {
                        let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                            Box::new(EventInterceptor::new(AddTodo(input)))
                        ];
                        context.queue.extend(interceptors);
                    },
                    Mode::Removing => {
                        let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                            Box::new(EventInterceptor::new(RemoveTodo(input)))
                        ];
                        context.queue.extend(interceptors);
                    },
                    Mode::Marking  => {
                        let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                            Box::new(EventInterceptor::new(MarkDone(input)))
                        ];
                        context.queue.extend(interceptors);
                    },
                }
            } else {
                context.effects.push(Box::new(db.mutate(move |state: &mut AppState| state.mode = Mode::Menu)));
                let dispatcher = context.coeffects.get::<Dispatcher<()>>().unwrap();
                context.effects.push(dispatcher.dispatch(ShowMenu));
            }
        }
        let input = *self;
        context.coeffects.insert(input);
        context.next()
    }
}

struct MenuInput;

impl Event<()> for MenuInput {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let input = context.coeffects.get::<Input>().unwrap();
            let next_mode = match input.0.as_ref() {
                "1" => {
                    let dispatcher = context.coeffects.get::<Dispatcher<()>>().unwrap();
                    context.effects.push(dispatcher.dispatch(ShowTodos));
                    Mode::Menu
                },
                "2" => Mode::Adding,
                "3" => Mode::Marking,
                "4" => Mode::Removing,
                _ => Mode::Menu,
            };
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            context.effects.push(Box::new(db.mutate(move |state: &mut AppState| state.mode = next_mode)));
        }
        context.next()
    }
}

struct ShowTodos;

impl Event<()> for ShowTodos {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            context.effects.push(Box::new(Print(format!("\nTODO:"))));
            let todos = &db.borrow().todos;
            if 0 == todos.len() {
                context.effects.push(Box::new(Print("  Nothing to do.".to_string())));
            }
            for (i, todo) in todos.iter().enumerate() {
                context.effects.push(Box::new(Print(format!("  - {}: [{}] {}", i, " ", todo))));
            }
            context.effects.push(Box::new(Print("".to_string())));
        }
        context.next()
    }
}

struct AddTodo(String);

impl Event<()> for AddTodo {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            context.effects.push(Box::new(db.mutate(move |state: &mut AppState| state.todos.push(self.0))));
        }
        context.next()
    }
}

struct RemoveTodo(String);

#[derive(Debug)]
enum RemoveError<E> {
    ParseError(E),
    OutOfRange(isize),
}

impl Event<()> for RemoveTodo {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            let max = db.borrow().todos.len();
            let index_res = self.0.parse::<isize>()
                .map_err(RemoveError::ParseError)
                .and_then(|index| {
                    if index >= 0 && (index as usize) < max {
                        Ok(index as usize)
                    } else {
                        Err(RemoveError::OutOfRange(index))
                    }
                });
            match index_res {
                Ok(index) => {
                    context.effects.push(Box::new(db.mutate(move |state: &mut AppState| {
                        state.todos.remove(index);
                    })));
                },
                Err(e) => {
                    context.effects.push(Box::new(Print(format!("Error removing: {:?}", e))))
                },
            };

        }
        context.next()
    }
}

struct MarkDone(String);

impl Event<()> for MarkDone {
    fn handle(self: Box<Self>, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        context.next()
    }
}

struct ShowPrompt;

impl Event<()> for ShowPrompt {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            let dispatcher = context.coeffects.get::<Dispatcher<()>>().unwrap();
            match db.borrow().mode {
                Mode::Menu => context.effects.push(dispatcher.dispatch(ShowMenu)),
                Mode::Adding => {
                    context.effects.push(dispatcher.dispatch(ShowTodos));
                },
                Mode::Removing => {
                    context.effects.push(dispatcher.dispatch(ShowTodos));
                },
                Mode::Marking => {
                    context.effects.push(dispatcher.dispatch(ShowTodos));
                },
            }
        }
        context.next()
    }
}

impl Interceptor for ShowPrompt {
    type Error = ();

    fn after(&self, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let dispatcher = context.coeffects.get::<Dispatcher<()>>().unwrap();
            context.effects.push(dispatcher.dispatch(ShowPrompt));
        }
        context.next()
    }
}

fn setup(app: &mut App<AppState>) {
    app.register_event::<ShowPrompt>();
    app.register_event::<ShowMenu>();
    app.register_event::<ShowTodos>();
    app.register_event_with::<Input>(vec![Box::new(ShowPrompt)]);
}

pub fn main() {

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let mut app = App::new(handle);
    setup(&mut app);

    app.dispatch(ShowMenu);

    let std_in_ch = spawn_stdin_stream_unbounded();
    core.run(std_in_ch.for_each(|m| {
        app.dispatch(Input(m));
        Ok(())
    })).unwrap();
}
