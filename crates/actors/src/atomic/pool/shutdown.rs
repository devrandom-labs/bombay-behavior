use crate::ShutdownId;

/// Checked worker-shutdown correlation shared by direct-worker pools.
pub(in crate::atomic) struct ShutdownSequence {
    next: Option<u64>,
}

impl ShutdownSequence {
    pub(in crate::atomic) const fn new() -> Self {
        Self { next: Some(0) }
    }

    pub(in crate::atomic) fn reserve(&mut self, count: usize) -> Option<Vec<ShutdownId>> {
        let mut next = self.next;
        let mut reserved = Vec::with_capacity(count);
        for _ in 0..count {
            let id = next?;
            reserved.push(ShutdownId(id));
            next = id.checked_add(1);
        }
        self.next = next;
        Some(reserved)
    }
}

#[cfg(test)]
mod tests {
    use crate::ShutdownId;

    use super::ShutdownSequence;

    #[test]
    fn reserves_ordered_nonreused_batches() {
        let mut shutdowns = ShutdownSequence::new();

        assert_eq!(shutdowns.reserve(0), Some(Vec::new()));
        assert_eq!(
            shutdowns.reserve(3),
            Some(vec![ShutdownId(0), ShutdownId(1), ShutdownId(2)])
        );
        assert_eq!(shutdowns.reserve(1), Some(vec![ShutdownId(3)]));
    }

    #[test]
    fn rejected_batch_preserves_the_next_id() {
        let mut shutdowns = ShutdownSequence {
            next: Some(u64::MAX),
        };

        assert_eq!(shutdowns.reserve(2), None);
        assert_eq!(shutdowns.reserve(1), Some(vec![ShutdownId(u64::MAX)]));
        assert_eq!(shutdowns.reserve(1), None);
    }
}
