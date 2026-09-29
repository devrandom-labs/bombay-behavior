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

        let empty_batch = shutdowns.reserve(0);
        assert_eq!(empty_batch, Some(Vec::new()));
        let first_batch = shutdowns.reserve(3);
        assert_eq!(
            first_batch,
            Some(vec![ShutdownId(0), ShutdownId(1), ShutdownId(2)])
        );
        let second_batch = shutdowns.reserve(1);
        assert_eq!(second_batch, Some(vec![ShutdownId(3)]));
    }

    #[test]
    fn rejected_batch_preserves_the_next_id() {
        let mut shutdowns = ShutdownSequence {
            next: Some(u64::MAX),
        };

        let oversized_batch = shutdowns.reserve(2);
        assert_eq!(oversized_batch, None);
        let final_batch = shutdowns.reserve(1);
        assert_eq!(final_batch, Some(vec![ShutdownId(u64::MAX)]));
        let exhausted_batch = shutdowns.reserve(1);
        assert_eq!(exhausted_batch, None);
    }
}
