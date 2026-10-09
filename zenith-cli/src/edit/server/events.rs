//! [`Hub`]: the Server-Sent Events fan-out.
//!
//! One thread owns every event stream. Workers hand it new streams and
//! events through a channel, so a slow client never blocks a command and
//! events reach every client in the order the document produced them.

use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::edit::doc::Event;

/// Time one event write may take before the client is dropped.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

enum Msg {
    /// A new stream, with its first event.
    Join(TcpStream, Event),
    /// An event for every stream.
    Send(Event),
    /// Send `shutdown` and close every stream.
    Stop,
}

/// The handle workers use to reach the event thread.
pub(crate) struct Hub {
    tx: Sender<Msg>,
    clients: Arc<AtomicUsize>,
    max: usize,
}

impl Hub {
    /// Start the event thread: at most `max` streams, a keep-alive comment
    /// every `ping`.
    pub(crate) fn start(max: usize, ping: Duration) -> std::io::Result<(Hub, JoinHandle<()>)> {
        let (tx, rx) = mpsc::channel::<Msg>();
        let clients = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&clients);
        let handle = std::thread::Builder::new()
            .name("zenith-edit-events".into())
            .spawn(move || {
                let mut streams: Vec<TcpStream> = Vec::new();
                let mut next_id: u64 = 1;
                loop {
                    match rx.recv_timeout(ping) {
                        Ok(Msg::Join(mut stream, first)) => {
                            let ok = stream.set_write_timeout(Some(WRITE_TIMEOUT)).is_ok()
                                && write_event(&mut stream, next_id, &first).is_ok();
                            next_id += 1;
                            if ok {
                                streams.push(stream);
                            } else {
                                count.fetch_sub(1, Ordering::SeqCst);
                            }
                        }
                        Ok(Msg::Send(event)) => {
                            let frame = frame(next_id, &event);
                            next_id += 1;
                            broadcast(&mut streams, frame.as_bytes(), &count);
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            broadcast(&mut streams, b": ping\n\n", &count);
                        }
                        Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => {
                            let event = Event::new("shutdown", serde_json::json!({}));
                            let frame = frame(next_id, &event);
                            for mut s in streams.drain(..) {
                                let _ = s.write_all(frame.as_bytes());
                                let _ = s.flush();
                                let _ = s.shutdown(Shutdown::Both);
                            }
                            count.store(0, Ordering::SeqCst);
                            break;
                        }
                    }
                }
            })?;
        Ok((Hub { tx, clients, max }, handle))
    }

    /// Reserve a stream slot. `false` when `max` streams are open.
    pub(crate) fn reserve(&self) -> bool {
        let mut n = self.clients.load(Ordering::SeqCst);
        loop {
            if n >= self.max {
                return false;
            }
            match self
                .clients
                .compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return true,
                Err(actual) => n = actual,
            }
        }
    }

    /// Hand `stream` (its response head already written) to the event
    /// thread, which sends `first` on it. Call after [`Hub::reserve`].
    pub(crate) fn join(&self, stream: TcpStream, first: Event) {
        if self.tx.send(Msg::Join(stream, first)).is_err() {
            self.clients.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Give back a slot from [`Hub::reserve`] whose stream never joined.
    pub(crate) fn release(&self) {
        self.clients.fetch_sub(1, Ordering::SeqCst);
    }

    /// Send `events` to every stream, in order.
    pub(crate) fn send(&self, events: Vec<Event>) {
        for event in events {
            let _ = self.tx.send(Msg::Send(event));
        }
    }

    /// Send `shutdown` to every stream and stop the event thread.
    pub(crate) fn stop(&self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

/// The wire form of one event.
fn frame(id: u64, event: &Event) -> String {
    format!("id: {id}\nevent: {}\ndata: {}\n\n", event.name, event.data)
}

fn write_event(stream: &mut TcpStream, id: u64, event: &Event) -> std::io::Result<()> {
    stream.write_all(frame(id, event).as_bytes())?;
    stream.flush()
}

/// Write `bytes` to every stream. A stream whose write fails is dropped.
fn broadcast(streams: &mut Vec<TcpStream>, bytes: &[u8], count: &AtomicUsize) {
    streams.retain_mut(|s| {
        let ok = s.write_all(bytes).and_then(|()| s.flush()).is_ok();
        if !ok {
            let _ = s.shutdown(Shutdown::Both);
            count.fetch_sub(1, Ordering::SeqCst);
        }
        ok
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_sse_records() {
        let e = Event::new("saved", serde_json::json!({ "version": 2 }));
        assert_eq!(
            frame(7, &e),
            "id: 7\nevent: saved\ndata: {\"version\":2}\n\n"
        );
    }

    #[test]
    fn reserve_stops_at_the_cap() {
        let (hub, handle) = Hub::start(2, Duration::from_secs(60)).expect("start");
        assert!(hub.reserve());
        assert!(hub.reserve());
        assert!(!hub.reserve());
        hub.stop();
        handle.join().expect("join");
    }
}
