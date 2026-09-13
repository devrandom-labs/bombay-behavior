//! Binding capacity and generation-exact application evidence.

use core::cmp::Ordering;
use core::num::NonZeroU64;
use std::sync::Arc;

use super::super::RoleName;
use super::super::capacity::{PositiveCapacity, ZeroCapacity};

/// Positive maximum number of retained key bindings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingCapacity {
    maximum: PositiveCapacity,
}

impl BindingCapacity {
    /// Validate the maximum number of retained bindings.
    ///
    /// # Errors
    ///
    /// Returns [`ZeroCapacity`] when `maximum` is zero.
    pub fn new(maximum: usize) -> Result<Self, ZeroCapacity> {
        PositiveCapacity::new(maximum).map(|maximum| Self { maximum })
    }

    const fn maximum(self) -> usize {
        self.maximum.get()
    }
}

/// Opaque non-reused identity of one retained key binding.
///
/// Applications receive generations from pool outcomes and cannot mint them:
///
/// ```compile_fail,E0599
/// use behavior_actors::atomic::BindingGeneration;
/// let _ = BindingGeneration::new(1);
/// ```
#[derive(Clone)]
pub struct BindingGeneration {
    ordinal: NonZeroU64,
    token: Arc<()>,
}

impl BindingGeneration {
    fn issued(ordinal: NonZeroU64, token: Arc<()>) -> Self {
        Self { ordinal, token }
    }

    /// Inspect the pool-local ordinal without gaining construction authority.
    #[must_use]
    pub const fn get(&self) -> u64 {
        self.ordinal.get()
    }
}

impl core::fmt::Debug for BindingGeneration {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("BindingGeneration")
            .field(&self.ordinal)
            .finish()
    }
}

impl PartialEq for BindingGeneration {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal && Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for BindingGeneration {}

/// Exact binding state expected by one rebalance or unbind command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingExpectation {
    /// The key must have no retained binding.
    Absent,
    /// The key must retain this exact generation.
    Exact(BindingGeneration),
}

/// Non-authorizing role and generation evidence carried by accepted work.
///
/// The value intentionally contains no key. Cloning it cannot create a binding
/// or extend key retention.
pub struct BindingEvidence<Role> {
    generation: BindingGeneration,
    role: RoleName<Role>,
}

impl<Role> BindingEvidence<Role> {
    fn issued(generation: BindingGeneration, role: RoleName<Role>) -> Self {
        Self { generation, role }
    }

    /// Borrow the opaque generation accepted for this work.
    #[must_use]
    pub const fn generation(&self) -> &BindingGeneration {
        &self.generation
    }

    /// Borrow the immutable semantic role selected at admission.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.role.role()
    }
}

impl<Role> Clone for BindingEvidence<Role> {
    fn clone(&self) -> Self {
        Self {
            generation: self.generation.clone(),
            role: self.role.clone(),
        }
    }
}

impl<Role> core::fmt::Debug for BindingEvidence<Role>
where
    Role: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BindingEvidence")
            .field("generation", &self.generation)
            .field("role", self.role())
            .finish()
    }
}

/// Caller-authored correlation echoed by one binding-management reply.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BindingRequestId(u64);

impl BindingRequestId {
    /// Name one rebalance or unbind command.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Inspect the caller-authored value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

struct Binding<Key, Role> {
    key: Key,
    evidence: BindingEvidence<Role>,
}

impl<Key, Role> Binding<Key, Role> {
    fn into_parts(self) -> (Key, BindingEvidence<Role>) {
        (self.key, self.evidence)
    }
}

struct GenerationSequence {
    next: Option<NonZeroU64>,
    token: Arc<()>,
}

impl GenerationSequence {
    fn new() -> Self {
        Self {
            next: Some(NonZeroU64::MIN),
            token: Arc::new(()),
        }
    }

    fn issue(&mut self) -> Option<BindingGeneration> {
        let ordinal = self.next?;
        self.next = ordinal.checked_add(1);
        Some(BindingGeneration::issued(ordinal, Arc::clone(&self.token)))
    }
}

pub(super) enum BindingReservationRejected<Key> {
    AlreadyBound(Key),
    CapacityExhausted(Key),
    GenerationsExhausted(Key),
}

pub(super) struct BindingTable<Key, Role> {
    capacity: BindingCapacity,
    bindings: Vec<Binding<Key, Role>>,
    generations: GenerationSequence,
}

impl<Key, Role> BindingTable<Key, Role>
where
    Key: Ord,
{
    pub(super) fn new(capacity: BindingCapacity) -> Self {
        Self {
            capacity,
            bindings: Vec::new(),
            generations: GenerationSequence::new(),
        }
    }

    pub(super) fn binding(&self, key: &Key) -> Option<&BindingEvidence<Role>> {
        self.position(key)
            .ok()
            .map(|position| &self.bindings[position].evidence)
    }

    pub(super) fn occupied(&mut self, key: &Key) -> Option<OccupiedBinding<'_, Key, Role>> {
        let position = self.position(key).ok()?;
        Some(OccupiedBinding {
            binding: &mut self.bindings[position],
            generations: &mut self.generations,
        })
    }

    pub(super) fn reserve(
        &mut self,
        key: Key,
        role: RoleName<Role>,
    ) -> Result<ReservedBinding<'_, Key, Role>, BindingReservationRejected<Key>> {
        let position = match self.position(&key) {
            Ok(_) => return Err(BindingReservationRejected::AlreadyBound(key)),
            Err(position) => position,
        };
        match self.bindings.len().cmp(&self.capacity.maximum()) {
            Ordering::Less => {}
            Ordering::Equal | Ordering::Greater => {
                return Err(BindingReservationRejected::CapacityExhausted(key));
            }
        }
        let generation = match self.generations.issue() {
            Some(generation) => generation,
            None => return Err(BindingReservationRejected::GenerationsExhausted(key)),
        };
        Ok(ReservedBinding {
            bindings: &mut self.bindings,
            position,
            key,
            generation,
            role,
        })
    }

    pub(super) fn remove(&mut self, key: &Key) -> Option<(Key, BindingEvidence<Role>)> {
        self.position(key)
            .ok()
            .map(|position| self.bindings.remove(position).into_parts())
    }

    pub(super) fn retire_role(&mut self, role: &Role) -> Vec<(Key, BindingEvidence<Role>)>
    where
        Role: Eq,
    {
        self.bindings
            .extract_if(.., |binding| binding.evidence.role() == role)
            .map(Binding::into_parts)
            .collect()
    }

    pub(super) fn drain(&mut self) -> Vec<(Key, BindingEvidence<Role>)> {
        self.bindings.drain(..).map(Binding::into_parts).collect()
    }

    fn position(&self, key: &Key) -> Result<usize, usize> {
        self.bindings
            .binary_search_by(|binding| binding.key.cmp(key))
    }
}

#[must_use = "a reserved binding must commit or return its key"]
pub(super) struct ReservedBinding<'table, Key, Role> {
    bindings: &'table mut Vec<Binding<Key, Role>>,
    position: usize,
    key: Key,
    generation: BindingGeneration,
    role: RoleName<Role>,
}

pub(super) struct OccupiedBinding<'table, Key, Role> {
    binding: &'table mut Binding<Key, Role>,
    generations: &'table mut GenerationSequence,
}

impl<Key, Role> OccupiedBinding<'_, Key, Role> {
    pub(super) fn rebind(
        self,
        role: RoleName<Role>,
    ) -> Option<(BindingEvidence<Role>, BindingEvidence<Role>)> {
        let generation = self.generations.issue()?;
        let current = BindingEvidence::issued(generation, role);
        let admitted = current.clone();
        let prior = core::mem::replace(&mut self.binding.evidence, current);
        Some((prior, admitted))
    }
}

impl<Key, Role> ReservedBinding<'_, Key, Role> {
    pub(super) fn commit(self) -> BindingEvidence<Role> {
        let evidence = BindingEvidence::issued(self.generation, self.role);
        let admitted = evidence.clone();
        self.bindings.insert(
            self.position,
            Binding {
                key: self.key,
                evidence,
            },
        );
        admitted
    }

    pub(super) fn reject(self) -> Key {
        self.key
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use std::sync::Arc;

    use super::{
        BindingCapacity, BindingEvidence, BindingGeneration, BindingReservationRejected,
        BindingTable, GenerationSequence,
    };
    use crate::atomic::RoleName;

    #[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct Account(u8);

    #[derive(Debug, Eq, PartialEq)]
    enum SearchRole {
        Primary,
        Replica,
    }

    fn bindings(maximum: usize) -> BindingTable<Account, SearchRole> {
        let capacity = BindingCapacity::new(maximum)
            .unwrap_or_else(|_| panic!("test binding capacity must be positive"));
        BindingTable::new(capacity)
    }

    fn commit(
        table: &mut BindingTable<Account, SearchRole>,
        account: Account,
        role: SearchRole,
    ) -> BindingEvidence<SearchRole> {
        table
            .reserve(account, RoleName::new(role))
            .unwrap_or_else(|_| panic!("test binding reservation must succeed"))
            .commit()
    }

    fn first_generation() -> BindingGeneration {
        BindingGeneration {
            ordinal: NonZeroU64::MIN,
            token: Arc::new(()),
        }
    }

    #[test]
    fn independent_pools_cannot_share_a_numeric_generation() {
        let left = first_generation();
        let right = first_generation();

        assert_eq!(left.get(), right.get());
        assert_ne!(left, right);
    }

    #[test]
    fn evidence_clones_identity_without_owning_a_key() {
        let evidence = BindingEvidence {
            generation: first_generation(),
            role: RoleName::new(String::from("search")),
        };
        let cloned = evidence.clone();

        assert_eq!(evidence.generation(), cloned.generation());
        assert_eq!(evidence.role(), "search");
        assert_eq!(cloned.role(), "search");
    }

    #[test]
    fn table_preserves_order_and_returns_rejected_keys_without_clone() {
        let mut table = bindings(2);
        let second = commit(&mut table, Account(2), SearchRole::Replica);
        let first = commit(&mut table, Account(1), SearchRole::Primary);

        let found_first = table
            .binding(&Account(1))
            .unwrap_or_else(|| panic!("first binding must remain"));
        let found_second = table
            .binding(&Account(2))
            .unwrap_or_else(|| panic!("second binding must remain"));
        assert_eq!(found_first.generation(), first.generation());
        assert_eq!(found_first.role(), first.role());
        assert_eq!(found_second.generation(), second.generation());
        assert_eq!(found_second.role(), second.role());

        let duplicate = table
            .reserve(Account(1), RoleName::new(SearchRole::Replica))
            .err()
            .unwrap_or_else(|| panic!("duplicate key must be rejected"));
        assert!(matches!(
            duplicate,
            BindingReservationRejected::AlreadyBound(Account(1))
        ));

        let full = table
            .reserve(Account(3), RoleName::new(SearchRole::Primary))
            .err()
            .unwrap_or_else(|| panic!("full table must reject the owned key"));
        assert!(matches!(
            full,
            BindingReservationRejected::CapacityExhausted(Account(3))
        ));

        let drained = table.drain();
        assert_eq!(
            drained
                .iter()
                .map(|(account, _)| account.0)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn rejected_admission_burns_its_generation_without_retaining_the_key() {
        let mut table = bindings(2);
        let rejected_key = table
            .reserve(Account(1), RoleName::new(SearchRole::Primary))
            .unwrap_or_else(|_| panic!("first reservation must succeed"))
            .reject();
        assert_eq!(rejected_key, Account(1));
        assert!(matches!(table.binding(&Account(1)), None));

        let evidence = commit(&mut table, Account(1), SearchRole::Primary);
        assert_eq!(evidence.generation().get(), 2);
    }

    #[test]
    fn generation_exhaustion_never_wraps_or_loses_the_key() {
        let maximum = NonZeroU64::new(u64::MAX)
            .unwrap_or_else(|| panic!("the maximum unsigned value is non-zero"));
        let mut sequence = GenerationSequence {
            next: Some(maximum),
            token: Arc::new(()),
        };
        assert_eq!(
            sequence
                .issue()
                .unwrap_or_else(|| panic!("the last generation remains issuable"))
                .get(),
            u64::MAX
        );
        assert!(matches!(sequence.issue(), None));

        let mut table = bindings(1);
        table.generations.next = None;
        let rejection = table
            .reserve(Account(9), RoleName::new(SearchRole::Primary))
            .err()
            .unwrap_or_else(|| panic!("exhaustion must reject the reservation"));
        assert!(matches!(
            rejection,
            BindingReservationRejected::GenerationsExhausted(Account(9))
        ));
    }

    #[test]
    fn role_retirement_and_unbind_return_complete_owned_bindings() {
        let mut table = bindings(3);
        commit(&mut table, Account(3), SearchRole::Primary);
        commit(&mut table, Account(1), SearchRole::Replica);
        commit(&mut table, Account(2), SearchRole::Primary);

        let retired = table.retire_role(&SearchRole::Primary);
        assert_eq!(
            retired
                .iter()
                .map(|(account, _)| account.0)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(matches!(table.binding(&Account(2)), None));
        assert!(matches!(table.binding(&Account(3)), None));

        let (account, evidence) = table
            .remove(&Account(1))
            .unwrap_or_else(|| panic!("remaining binding must be removable"));
        assert_eq!(account, Account(1));
        assert_eq!(evidence.role(), &SearchRole::Replica);
        assert!(matches!(table.binding(&Account(1)), None));
    }

    #[test]
    fn role_change_commits_once_without_removing_the_binding() {
        let mut table = bindings(1);
        let original = commit(&mut table, Account(1), SearchRole::Primary);

        let (prior, current) = table
            .occupied(&Account(1))
            .unwrap_or_else(|| panic!("the binding must remain occupied"))
            .rebind(RoleName::new(SearchRole::Replica))
            .unwrap_or_else(|| panic!("a fresh generation must remain"));

        assert_eq!(prior.generation(), original.generation());
        assert_eq!(prior.role(), &SearchRole::Primary);
        assert_eq!(current.generation().get(), 2);
        assert_eq!(current.role(), &SearchRole::Replica);
        let retained = table
            .binding(&Account(1))
            .unwrap_or_else(|| panic!("replacement cannot expose absence"));
        assert_eq!(retained.generation(), current.generation());
        assert_eq!(retained.role(), current.role());
    }

    #[test]
    fn exhausted_role_change_preserves_the_current_binding() {
        let mut table = bindings(1);
        let original = commit(&mut table, Account(1), SearchRole::Primary);
        table.generations.next = None;

        let replacement = table
            .occupied(&Account(1))
            .unwrap_or_else(|| panic!("the binding must remain occupied"))
            .rebind(RoleName::new(SearchRole::Replica));
        assert!(matches!(replacement, None));

        let retained = table
            .binding(&Account(1))
            .unwrap_or_else(|| panic!("exhaustion cannot remove the binding"));
        assert_eq!(retained.generation(), original.generation());
        assert_eq!(retained.role(), &SearchRole::Primary);
    }
}
