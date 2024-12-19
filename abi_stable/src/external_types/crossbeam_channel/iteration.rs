use super::*;

use std::iter::FusedIterator;

///////////////////////////////////////////////////////////////////////////////

/// An iterator which receives the values sent through the channel,
/// blocking until a value is received.
///
/// If the channel is disconnected this will return None without blocking.
pub struct RIter<'a, T> {
    pub(super) channel: &'a RReceiver<T>,
}

impl<T> Iterator for RIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.channel.recv().ok()
    }
}

impl<T> Debug for RIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("RIter{..}")
    }
}

///////////////////////////////////////////////////////////////////////////////

/// An iterator which receives the values sent through the channel,
/// blocking until a value is received.
///
/// If the channel is disconnected this will return None without blocking.
pub struct RIntoIter<T> {
    pub(super) channel: RReceiver<T>,
}

impl<T> FusedIterator for RIntoIter<T> {}

impl<T> Iterator for RIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.channel.recv().ok()
    }
}

impl<T> Debug for RIntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("RIntoIter{..}")
    }
}
