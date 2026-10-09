//! Page scheduling: the [`PageRunner`] trait, [`Sequential`], and the
//! order-keeping map the render entry points use.

use std::sync::Mutex;

/// Runs one job per page. A native host can run jobs on a thread pool; the
/// pipeline itself never starts a thread.
pub trait PageRunner {
    /// Call `job(i)` once for every `i` in `0..count`, in any order, on any
    /// thread. Return after every call has finished.
    fn run(&self, count: usize, job: &(dyn Fn(usize) + Sync));
}

/// Runs every job in order on the calling thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sequential;

impl PageRunner for Sequential {
    fn run(&self, count: usize, job: &(dyn Fn(usize) + Sync)) {
        for index in 0..count {
            job(index);
        }
    }
}

/// Run `f` for every index in `0..count` through `runner` and return the
/// results in index order.
///
/// Output never depends on scheduling. One or zero items run on this thread.
/// An index the runner skipped runs here afterwards, so every slot is filled.
pub(crate) fn map_pages<T, F>(runner: &dyn PageRunner, count: usize, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    if count <= 1 {
        return (0..count).map(f).collect();
    }
    let slots: Vec<Mutex<Option<T>>> = (0..count).map(|_| Mutex::new(None)).collect();
    runner.run(count, &|index| {
        let value = f(index);
        if let Some(Ok(mut slot)) = slots.get(index).map(Mutex::lock) {
            *slot = Some(value);
        }
    });
    slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| slot.into_inner().ok().flatten().unwrap_or_else(|| f(index)))
        .collect()
}

/// Run `f` on every item of `items` through `runner` (see [`map_pages`]) and
/// return the results in item order.
pub(crate) fn map_slice<I, T, F>(runner: &dyn PageRunner, items: &[I], f: F) -> Vec<T>
where
    I: Sync,
    T: Send,
    F: Fn(&I) -> T + Sync,
{
    map_pages(runner, items.len(), |i| items.get(i).map(&f))
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs jobs in reverse order and skips index 2, to prove order and fill.
    struct Reversed;

    impl PageRunner for Reversed {
        fn run(&self, count: usize, job: &(dyn Fn(usize) + Sync)) {
            for index in (0..count).rev().filter(|&i| i != 2) {
                job(index);
            }
        }
    }

    #[test]
    fn map_keeps_index_order_under_any_schedule() {
        let expected: Vec<usize> = (0..16).map(|i| i * 3).collect();
        assert_eq!(map_pages(&Sequential, 16, |i| i * 3), expected);
        assert_eq!(map_pages(&Reversed, 16, |i| i * 3), expected);
    }

    #[test]
    fn map_handles_zero_and_one_item() {
        assert!(map_pages(&Sequential, 0, |i| i).is_empty());
        assert_eq!(map_pages(&Sequential, 1, |i| i + 7), vec![7]);
        assert_eq!(
            map_slice(&Reversed, &[1, 2, 3], |x| x * 10),
            vec![10, 20, 30]
        );
    }
}
