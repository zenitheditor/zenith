//! [`Shared`]: the state every server thread reaches.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::events::Hub;
use super::guard::Guard;
use super::images::ImageCache;
use crate::edit::doc::DocState;
use crate::edit::http::Limits;

/// One server's document, image cache, event hub, and request checks.
pub(crate) struct Shared {
    pub(crate) doc: Mutex<DocState>,
    pub(crate) images: Mutex<ImageCache>,
    pub(crate) hub: Hub,
    pub(crate) guard: Guard,
    pub(crate) limits: Limits,
    /// The bound address.
    pub(crate) addr: SocketAddr,
    stop: AtomicBool,
}

impl Shared {
    pub(crate) fn new(doc: DocState, hub: Hub, guard: Guard, addr: SocketAddr) -> Self {
        Self {
            doc: Mutex::new(doc),
            images: Mutex::new(ImageCache::default()),
            hub,
            guard,
            limits: Limits::default(),
            addr,
            stop: AtomicBool::new(false),
        }
    }

    /// The document, locked. A panicked holder cannot leave it half
    /// changed (every change is a whole-session swap), so a poisoned lock
    /// is taken as is.
    pub(crate) fn doc(&self) -> MutexGuard<'_, DocState> {
        self.doc.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The image cache, locked.
    pub(crate) fn images(&self) -> MutexGuard<'_, ImageCache> {
        self.images.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `true` once shutdown started.
    pub(crate) fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Start shutdown: stop the event hub and wake the accept loop.
    pub(crate) fn request_stop(&self) {
        if self.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        self.hub.stop();
        let wake = match self.addr {
            SocketAddr::V4(a) if a.ip().is_unspecified() => {
                SocketAddr::from(([127, 0, 0, 1], a.port()))
            }
            SocketAddr::V6(a) if a.ip().is_unspecified() => {
                SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, a.port()))
            }
            other => other,
        };
        let _ = std::net::TcpStream::connect_timeout(&wake, std::time::Duration::from_secs(1));
    }
}
