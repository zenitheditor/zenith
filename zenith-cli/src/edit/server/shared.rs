//! [`Shared`]: the state every server thread reaches.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::events::Hub;
use super::guard::Guard;
use super::images::ImageCache;
use super::slots::Slots;
use crate::edit::browser::LaunchFile;
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
    /// Open-connection caps.
    pub(crate) slots: Arc<Slots>,
    /// The browser redirect file, until the first authenticated request.
    pub(crate) launch: Mutex<Option<LaunchFile>>,
    /// A stop signal arrived while the session had unsaved edits. The next
    /// one discards them. Cleared when the session is clean again.
    pub(crate) stop_armed: AtomicBool,
    stop: AtomicBool,
}

impl Shared {
    pub(crate) fn new(
        doc: DocState,
        hub: Hub,
        guard: Guard,
        addr: SocketAddr,
        slots: Arc<Slots>,
    ) -> Self {
        Self {
            doc: Mutex::new(doc),
            images: Mutex::new(ImageCache::default()),
            hub,
            guard,
            limits: Limits::default(),
            addr,
            slots,
            launch: Mutex::new(None),
            stop_armed: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        }
    }

    /// A request proved it holds the token: the browser redirect file has
    /// done its job, so remove it.
    pub(crate) fn authenticated(&self) {
        self.drop_launch();
    }

    /// Keep the redirect file until [`Shared::authenticated`] or shutdown.
    pub(crate) fn keep_launch(&self, file: LaunchFile) {
        *self.launch.lock().unwrap_or_else(PoisonError::into_inner) = Some(file);
    }

    /// Remove the redirect file, if any.
    pub(crate) fn drop_launch(&self) {
        let file = self
            .launch
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(file);
    }

    /// Disarm the pending stop once the session is clean again.
    pub(crate) fn note_clean(&self, dirty: bool) {
        if !dirty {
            self.stop_armed.store(false, Ordering::SeqCst);
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
