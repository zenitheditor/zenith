//! [`Slots`]: the cap on open connections.
//!
//! Every accepted connection takes a slot for as long as its thread runs.
//! Past the cap the accept loop answers 503 without blocking. A peer that
//! is not loopback also has a cap of its own, so one remote host cannot
//! take every slot (with `--allow-remote`).

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Counts {
    total: usize,
    /// Open connections per non-loopback peer.
    peers: BTreeMap<IpAddr, usize>,
}

/// Open-connection counters.
pub(crate) struct Slots {
    counts: Mutex<Counts>,
    released: Condvar,
    max: usize,
    per_peer: usize,
}

/// One taken slot. Dropping it gives the slot back.
pub(crate) struct Slot {
    slots: Arc<Slots>,
    /// The peer counted under the per-peer cap.
    peer: Option<IpAddr>,
}

impl Slots {
    /// At most `max` connections, at most `per_peer` from one non-loopback
    /// peer.
    pub(crate) fn new(max: usize, per_peer: usize) -> Arc<Self> {
        Arc::new(Self {
            counts: Mutex::new(Counts::default()),
            released: Condvar::new(),
            max,
            per_peer,
        })
    }

    fn counts(&self) -> MutexGuard<'_, Counts> {
        self.counts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Take a slot for a connection from `peer`. `None` past a cap.
    pub(crate) fn acquire(self: &Arc<Self>, peer: IpAddr) -> Option<Slot> {
        let mut counts = self.counts();
        if counts.total >= self.max {
            return None;
        }
        let counted = (!peer.is_loopback()).then_some(peer);
        if let Some(ip) = counted {
            let n = counts.peers.entry(ip).or_insert(0);
            if *n >= self.per_peer {
                return None;
            }
            *n += 1;
        }
        counts.total += 1;
        Some(Slot {
            slots: Arc::clone(self),
            peer: counted,
        })
    }

    /// Wait until every slot is back, at most `limit`. `true` when idle.
    pub(crate) fn wait_idle(&self, limit: Duration) -> bool {
        let end = Instant::now() + limit;
        let mut counts = self.counts();
        while counts.total > 0 {
            let Some(left) = end.checked_duration_since(Instant::now()) else {
                return false;
            };
            counts = match self.released.wait_timeout(counts, left) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        true
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut counts = self.slots.counts();
        counts.total = counts.total.saturating_sub(1);
        if let Some(ip) = self.peer
            && let Some(n) = counts.peers.get_mut(&ip)
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                counts.peers.remove(&ip);
            }
        }
        drop(counts);
        self.slots.released.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_hold_and_released_slots_return() {
        let slots = Slots::new(3, 1);
        let local: IpAddr = "127.0.0.1".parse().expect("ip");
        let remote: IpAddr = "10.0.0.7".parse().expect("ip");
        let a = slots.acquire(local).expect("a");
        let b = slots.acquire(remote).expect("b");
        assert!(slots.acquire(remote).is_none(), "per-peer cap");
        let c = slots.acquire(local).expect("loopback has no per-peer cap");
        assert!(slots.acquire(local).is_none(), "total cap");
        drop(b);
        assert!(slots.acquire(remote).is_some(), "the peer slot came back");
        drop((a, c));
        assert!(slots.wait_idle(Duration::from_millis(10)));
    }

    #[test]
    fn wait_idle_ends_when_the_last_slot_returns() {
        let slots = Slots::new(2, 2);
        let slot = slots
            .acquire("127.0.0.1".parse().expect("ip"))
            .expect("slot");
        assert!(!slots.wait_idle(Duration::from_millis(20)));
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            drop(slot);
        });
        assert!(slots.wait_idle(Duration::from_secs(5)));
        t.join().expect("join");
    }
}
