extern crate futures;
extern crate tokio_core;
extern crate tokio_interceptor;

use std::io::{self, BufRead};
use std::thread;

use futures::stream::iter_result;
use futures::{Future, Sink, Stream};
use futures::sync::mpsc::{unbounded, SendError, UnboundedReceiver};
use tokio_core::reactor::{Core, Handle};
use tokio_interceptor::{Context, Db, Effect, Event,
                        EventDispatcher, HandleEffects,
                        InjectCoeffect, Interceptor};

struct App<State> {
    handle: Handle,
    db: Db<State>,
    dispatcher: EventDispatcher<()>,
}

impl<State> App<State>
where State: 'static + Default,
{
    pub fn new(handle: Handle) -> App<State> {
        App { handle,
              db: Db::new(State::default()),
              dispatcher: EventDispatcher::new() }
    }

    fn default_interceptors(&self) -> Vec<Box<Interceptor<Error = ()>>> {
        let inject_state = InjectCoeffect::<Db<State>, ()>::new(self.db.clone());
        let handle_effects = HandleEffects::new();
        vec![Box::new(inject_state), Box::new(handle_effects)]
    }

    pub fn register_event<E: 'static + Event<()>>(&mut self) {
        self.register_event_with::<E>(vec![]);
    }

    pub fn register_event_with<E: 'static + Event<()>>(&mut self, mut interceptors: Vec<Box<Interceptor<Error = ()>>>) {
        let mut i = self.default_interceptors();
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

enum Mode {
    Adding, Removing, Marking, Menu
}

struct AppState {
    mode: Mode,
}

impl Default for AppState {
    fn default() -> AppState {
        AppState { mode: Mode::Menu }
    }
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
        context.next()
    }
}

struct Input(String);

impl Event<()> for Input {
    fn handle(&self, mut context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        context.push_effect(Print(self.0.clone()));
        context.next()
    }
}

fn setup(app: &mut App<AppState>) {
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
