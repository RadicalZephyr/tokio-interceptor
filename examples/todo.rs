extern crate futures;
extern crate tokio_core;
extern crate tokio_interceptor;

use std::io::{self, BufRead};
use std::thread;

use futures::stream::iter_result;
use futures::{future, Future, Sink, Stream};
use futures::sync::mpsc::{unbounded, SendError, UnboundedReceiver};
use tokio_core::reactor::Core;
use tokio_interceptor::{Context, Event, EventDispatcher};

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

struct Input(String);

impl Event<()> for Input {
    fn handle(&self, context: Context<()>) -> Box<Future<Item = Context<()>, Error = ()>> {
        println!("{:?}", self.0);
        Box::new(future::ok(context))
    }
}

fn setup(app: &mut EventDispatcher<()>) {
    app.register_event::<Input>(vec![]);
}

pub fn main() {
    let mut app = EventDispatcher::new();
    setup(&mut app);

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let std_in_ch = spawn_stdin_stream_unbounded();
    core.run(std_in_ch.for_each(|m| {
        handle.spawn(app.dispatch(Input(m)).map(|_| ()).map_err(|_| ()));
        Ok(())
    })).unwrap();
}
