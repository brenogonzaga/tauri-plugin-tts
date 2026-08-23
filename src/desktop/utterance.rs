use std::collections::VecDeque;
use std::sync::Mutex;

/// In-flight utterance IDs, oldest first.
///
/// A single slot cannot express this: under [`QueueMode::Add`] two utterances are in flight
/// at once, and a lone slot made the first one's finish event report the second one's ID.
///
/// [`QueueMode::Add`]: crate::models::QueueMode::Add
#[derive(Debug, Default)]
pub struct UtteranceQueue(Mutex<VecDeque<String>>);

impl UtteranceQueue {
    pub fn push(&self, id: String) {
        if let Ok(mut queue) = self.0.lock() {
            queue.push_back(id);
        }
    }

    /// The utterance the engine is starting: the oldest one still in flight. Peeks, because
    /// the utterance stays in flight until it ends or is stopped.
    pub fn begin(&self) -> Option<String> {
        self.0.lock().ok().and_then(|queue| queue.front().cloned())
    }

    /// The utterance the engine just ended or stopped, removed from the queue.
    pub fn finish(&self) -> Option<String> {
        self.0.lock().ok().and_then(|mut queue| queue.pop_front())
    }

    /// Drops an utterance the engine refused, so it is not popped later by another
    /// utterance's callback and reported under the wrong ID.
    pub fn forget(&self, id: &str) {
        if let Ok(mut queue) = self.0.lock() {
            if let Some(index) = queue.iter().rposition(|queued| queued == id) {
                queue.remove(index);
            }
        }
    }

    pub fn drain(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default()
    }

    /// Puts a drained batch back at the front, oldest first, when the stop it was draining
    /// for never happened.
    pub fn restore(&self, ids: Vec<String>) {
        if let Ok(mut queue) = self.0.lock() {
            for id in ids.into_iter().rev() {
                queue.push_front(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_of(ids: &[&str]) -> UtteranceQueue {
        let queue = UtteranceQueue::default();
        for id in ids {
            queue.push((*id).into());
        }
        queue
    }

    /// With a single slot, A's completion was reported under B's ID and B then finished
    /// twice, so a caller awaiting finish{A} waited forever.
    #[test]
    fn queued_utterances_finish_under_their_own_ids() {
        let queue = queue_of(&["A"]);
        assert_eq!(queue.begin().as_deref(), Some("A"));

        queue.push("B".into());
        assert_eq!(queue.finish().as_deref(), Some("A"));
        assert_eq!(queue.begin().as_deref(), Some("B"));
        assert_eq!(queue.finish().as_deref(), Some("B"));
        assert_eq!(queue.finish(), None);
    }

    /// Under `flush` the stop callback for the outgoing utterance can land on either side of
    /// the replacement being enqueued. Both orders must cancel the one that was replaced.
    #[test]
    fn a_flushed_utterance_is_cancelled_under_its_own_id() {
        for stop_fires_after_enqueue in [false, true] {
            let queue = queue_of(&["A"]);
            queue.begin();

            if stop_fires_after_enqueue {
                queue.push("B".into());
                assert_eq!(queue.finish().as_deref(), Some("A"));
            } else {
                assert_eq!(queue.finish().as_deref(), Some("A"));
                queue.push("B".into());
            }

            assert_eq!(queue.begin().as_deref(), Some("B"));
        }
    }

    /// `stop` drains first so each pending utterance is cancelled exactly once under its own
    /// ID; the engine callback then finds an empty queue and stays quiet.
    #[test]
    fn draining_reports_every_pending_utterance_once() {
        let queue = queue_of(&["A", "B"]);

        assert_eq!(queue.drain(), ["A", "B"]);
        assert_eq!(queue.finish(), None);
        assert!(queue.drain().is_empty());
    }

    #[test]
    fn a_failed_stop_puts_the_utterances_back_in_order() {
        let queue = queue_of(&["A", "B"]);

        let drained = queue.drain();
        queue.restore(drained);

        assert_eq!(queue.finish().as_deref(), Some("A"));
        assert_eq!(queue.finish().as_deref(), Some("B"));
        assert_eq!(queue.finish(), None);
    }

    #[test]
    fn a_refused_utterance_leaves_nothing_behind() {
        let queue = queue_of(&["A", "B"]);
        queue.forget("B");

        assert_eq!(queue.finish().as_deref(), Some("A"));
        assert_eq!(queue.finish(), None);
    }

    #[test]
    fn forgetting_an_unknown_id_is_a_no_op() {
        let queue = queue_of(&["A"]);
        queue.forget("Z");

        assert_eq!(queue.finish().as_deref(), Some("A"));
    }
}
