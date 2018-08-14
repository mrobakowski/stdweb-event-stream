//! This crate provides [`event_stream`](IEventTargetExt::event_stream) function for
//! [`IEventTarget`](::stdweb::web::IEventTarget). The function simply returns a
//! [`Stream`](::futures_core::Stream) of given [`ConcreteEvent`](::stdweb::web::event::ConcreteEvent).
//!
//! ```rust
//! PromiseFuture::spawn_local(
//!     document().event_stream().for_each(|e: ClickEvent| {
//!         console!(log, e);
//!         future::ok(())
//!     }).map(|_|())
//! );
//! ```

#[cfg_attr(test, macro_use)]
extern crate stdweb;
extern crate futures_core;
extern crate futures_executor;
extern crate futures_channel;
extern crate futures_util;

use ::futures_channel::mpsc::{unbounded, UnboundedReceiver};
use ::futures_core::{Stream, Never, task::Context, Async};
use ::futures_util::SinkExt;
use ::futures_util::FutureExt;
use ::stdweb::web::event::ConcreteEvent;
use ::stdweb::web::{IEventTarget, EventListenerHandle};
use ::stdweb::PromiseFuture;

pub struct EventStream<E> {
    stream: UnboundedReceiver<E>,
    event_listener_handle: Option<EventListenerHandle>,
}

pub trait IEventTargetExt: IEventTarget {
    fn event_stream<E: ConcreteEvent + 'static>(&self) -> EventStream<E> {
        let (tx, rx) = unbounded();
        let handle = self.add_event_listener(move |e: E| {
            // is this the right function to call? it says in the docs, that it is usually not
            // needed to call it more than once in main...
            PromiseFuture::spawn_local(
                // I think we could get rid of `clone` by polling correctly? dunno
                tx.clone().send(e)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
                    .map_err(PromiseFuture::print_error_panic)
            )
        });

        EventStream {
            stream: rx,
            event_listener_handle: Some(handle),
        }
    }
}

impl<T: IEventTarget> IEventTargetExt for T {}

impl<E> Stream for EventStream<E> {
    type Item = E;
    type Error = Never;

    fn poll_next(&mut self, cx: &mut Context) -> Result<Async<Option<E>>, Never> {
        self.stream.poll_next(cx)
    }
}

impl<E> Drop for EventStream<E> {
    fn drop(&mut self) {
        self.event_listener_handle.take().map(|h| h.remove());
    }
}

#[cfg(test)]
mod test {
    use ::stdweb::web::*;
    use ::stdweb::web::event::*;
    use ::stdweb::unstable::TryInto;
    use ::futures_util::StreamExt;
    use super::*;

    #[test]
    #[allow(unreachable_code, unused_variables)] // TODO: remove this once you figure out how to test this
    fn test_event_stream() {
        let document = document();

        let e: ClickEvent = js!(return new MouseEvent("click")).try_into().unwrap();

        let stream = document.event_stream::<ClickEvent>();

        document.dispatch_event(&e).unwrap();
        document.dispatch_event(&e).unwrap();
        document.dispatch_event(&e).unwrap();

        return; // TODO: remove this once you figure out how to test this
        // The line following this comment causes panics on emscripten target, because of thread
        // support. It's impossible to properly test this anyway, because the test would have to be
        // somehow interleaved with the js event loop. Using LocalPool with LocalExecutor won't
        // work, because the event_listener won't be ever called, because js event loop is stuck at
        // our test.
        // At least I think so. If you, dear reader, have an idea how to test this, please submit a
        // merge request or just share your idea.

        let v = ::futures_executor::block_on(stream.take(3).collect::<Vec<_>>()).unwrap();

        assert_eq!(v, vec![e.clone(), e.clone(), e.clone()])
    }
}