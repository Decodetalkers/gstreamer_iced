use std::hash::Hash;

use std::ops::Deref;
use std::sync::atomic::{self, AtomicU64};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// The id of the window.
///
/// Internally Iced reserves `window::Id::MAIN` for the first window spawned.
pub struct Id(pub u64);

static COUNT: AtomicU64 = AtomicU64::new(0);

impl Id {
    /// Creates a new unique window [`Id`].
    pub fn unique() -> Id {
        Id(COUNT.fetch_add(1, atomic::Ordering::Relaxed))
    }
}

impl Deref for Id {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
