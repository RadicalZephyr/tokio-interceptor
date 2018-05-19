extern crate futures;
extern crate tokio_core;
extern crate tokio_interceptor;

use std::io::{self, BufRead};
use std::thread;

use futures::stream::iter_result;
use futures::{future, Future, Sink, Stream};
use futures::sync::mpsc::{unbounded, SendError, UnboundedReceiver};
use tokio_core::reactor::{Core, Handle};
use tokio_interceptor::{Context, Effect, Event, EventDispatcher,
                        HandleEffects, Interceptor};

struct App {
    handle: Handle,
    dispatcher: EventDispatcher<()>,
}

impl App {
    pub fn new(handle: Handle) -> App {
        App { handle, dispatcher: EventDispatcher::new() }
    }

    fn default_interceptors() -> Vec<Box<Interceptor<Error = ()>>> {
        vec![Box::new(HandleEffects::new())]
    }

    pub fn register_event<E: 'static + Event<()>>(&mut self) {
        self.register_event_with::<E>(vec![]);
    }

    pub fn register_event_with<E: 'static + Event<()>>(&mut self, mut interceptors: Vec<Box<Interceptor<Error = ()>>>) {
        let mut i = App::default_interceptors();
        i.append(&mut interceptors);

        self.dispatcher.register_event::<E>(i);
    }

    pub fn dispatch<E: 'static + Event<()>>(&mut self, e: E) {
        self.handle.spawn(self.dispatcher.dispatch(e).map(|_| ()).map_err(|_| ()));
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

struct Print(String);

impl Effect for Print {
    fn action(&mut self) {
        println!("{}", self.0);
    }
}

struct ShowMenu;

impl Event<()> for ShowMenu {
    fn handle(&self, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
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
        Box::new(future::ok(context))
    }
}

struct Input(String);

impl Event<()> for Input {
    fn handle(&self, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        context.push_effect(Print(self.0.clone()));
        Box::new(future::ok(context))
    }
}

fn setup(app: &mut App) {
    app.register_event::<Input>();
    app.register_event::<ShowMenu>();
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
