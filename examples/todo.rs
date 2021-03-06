extern crate futures;
extern crate tokio_core;
extern crate tokio_interceptor;


use std::{io, thread};
use std::io::BufRead;

use futures::stream::iter_result;
use futures::{future, Future, Sink, Stream};
use futures::sync::mpsc::{unbounded, SendError, UnboundedReceiver};
use tokio_core::reactor::Core;
use tokio_interceptor::{App, Context, Db, Dispatcher, Effect,
                        Event, EventInterceptor, Interceptor};

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

#[derive(Copy, Clone, Debug)]
enum Mode {
    Adding, Removing, Marking, Menu, Quitting,
}

struct AppState {
    mode: Mode,
    todos: Vec<(bool, String)>,
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
3 - Change task "done" status
4 - Remove a task
---
"#);
        context.push_effect(Print(menu));
        context.next()
    }
}

struct Input(String);

impl Event<()> for Input {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            match db.borrow().mode {
                Mode::Menu     => {
                    let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                        Box::new(EmptyInputHandler(Mode::Quitting)),
                        Box::new(EventInterceptor::new(MenuInput))
                    ];
                    context.queue.extend(interceptors);
                },
                Mode::Adding   => {
                    let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                        Box::new(EmptyInputHandler(Mode::Menu)),
                        Box::new(EventInterceptor::new(AddTodo))
                    ];
                    context.queue.extend(interceptors);
                },
                Mode::Removing => {
                    let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                        Box::new(EmptyInputHandler(Mode::Menu)),
                        Box::new(ParseIndex),
                        Box::new(EventInterceptor::new(RemoveTodo))
                    ];
                    context.queue.extend(interceptors);
                },
                Mode::Marking  => {
                    let interceptors: Vec<Box<Interceptor<Error = ()>>> = vec![
                        Box::new(EmptyInputHandler(Mode::Menu)),
                        Box::new(ParseIndex),
                        Box::new(EventInterceptor::new(ToggleMark))
                    ];
                    context.queue.extend(interceptors);
                },
                Mode::Quitting => context.queue.push_back(Box::new(EventInterceptor::new(Quit(0))) as Box<Interceptor<Error = ()>>),
            }
        }
        let input = *self;
        context.coeffects.insert(input);
        context.next()
    }
}

struct NonEmptyInput(String);

struct EmptyInputHandler(Mode);

impl Interceptor for EmptyInputHandler {
    type Error = ();

    fn before(&self, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        match context.coeffects.remove::<Input>().unwrap().0.as_ref() {
            "" => {
                context.queue.clear();
                let db = context.coeffects.get::<Db<AppState>>().unwrap();
                let mode = self.0;
                context.effects.push(Box::new(db.mutate(move |state: &mut AppState| state.mode = mode)));
                let dispatcher = context.coeffects.get::<Dispatcher<()>>().unwrap();
                context.effects.push(dispatcher.dispatch(ShowMenu));
            },
            input => {
                context.coeffects.insert(NonEmptyInput(input.to_string()));
            }
        };

        context.next()
    }
}

struct Index(Result<usize, RemoveError<std::num::ParseIntError>>);

struct ParseIndex;

impl Interceptor for ParseIndex {
    type Error = ();

    fn before(&self, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        let max = {
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            db.borrow().todos.len()
        };
        let index_res = context.coeffects.remove::<NonEmptyInput>().unwrap()
            .0.parse::<isize>()
            .map_err(RemoveError::ParseError)
            .and_then(|index| {
                if index >= 0 && (index as usize) < max {
                    Ok(index as usize)
                } else {
                    Err(RemoveError::OutOfRange(index))
                }
            });
        context.coeffects.insert(Index(index_res));
        context.next()
    }
}

struct MenuInput;

impl Event<()> for MenuInput {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let input = context.coeffects.get::<NonEmptyInput>().unwrap();
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
                let status = if todo.0 { "✔" } else { " " };
                context.effects.push(Box::new(Print(format!("  - {}: [{}] {}", i, status, todo.1))));
            }
            context.effects.push(Box::new(Print("".to_string())));
        }
        context.next()
    }
}

struct AddTodo;

impl Event<()> for AddTodo {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let input = context.coeffects.remove::<NonEmptyInput>().unwrap().0;
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            context.effects.push(Box::new(db.mutate(move |state: &mut AppState| state.todos.push((false, input)))));
        }
        context.next()
    }
}

struct RemoveTodo;

#[derive(Debug)]
enum RemoveError<E> {
    ParseError(E),
    OutOfRange(isize),
}

impl Event<()> for RemoveTodo {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let index_res = context.coeffects.remove::<Index>().unwrap().0;
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
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

struct ToggleMark;

impl Event<()> for ToggleMark {
    fn handle(self: Box<Self>, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        {
            let index_res = context.coeffects.remove::<Index>().unwrap().0;
            let db = context.coeffects.get::<Db<AppState>>().unwrap();
            match index_res {
                Ok(index) => {
                    context.effects.push(Box::new(db.mutate(move |state: &mut AppState| {
                        let todo = state.todos.get_mut(index).unwrap();
                        todo.0 = ! todo.0;
                    })));
                },
                Err(e) => {
                    context.effects.push(Box::new(Print(format!("Error marking: {:?}", e))))
                },
            };
        }
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
                Mode::Quitting => {
                    context.queue.push_back(Box::new(EventInterceptor::new(Quit(0))) as Box<Interceptor<Error = ()>>);
                }
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

struct Quit(i64);

impl Event<()> for Quit {
    fn handle(self: Box<Self>, _context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        Box::new(future::err(()))
    }
}

fn setup(app: &mut App<AppState>) {
    app.register_event::<ShowPrompt>();
    app.register_event::<ShowMenu>();
    app.register_event::<ShowTodos>();
    app.register_event_with::<Input>(vec![Box::new(ShowPrompt)]);
}

pub fn main() -> Result<(), ()> {

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let mut app = App::new(handle);
    setup(&mut app);

    let handle = core.handle();
    handle.spawn(app.dispatch(ShowMenu).map(|_| ()).map_err(|_| ()));

    let std_in_ch = spawn_stdin_stream_unbounded();
    core.run(std_in_ch.for_each(|m| {
        app.dispatch(Input(m)).map(|_| ()).map_err(|_| ())
    }))?;

    Ok(())
}
