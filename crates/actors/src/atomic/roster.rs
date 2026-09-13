//! Non-empty immutable worker-role order shared by atomic rosters.

use std::sync::Arc;

pub(crate) struct RoleName<Role> {
    role: Arc<Role>,
}

impl<Role> RoleName<Role> {
    pub(crate) fn new(role: Role) -> Self {
        Self {
            role: Arc::new(role),
        }
    }

    pub(crate) fn role(&self) -> &Role {
        self.role.as_ref()
    }
}

impl<Role> Clone for RoleName<Role> {
    fn clone(&self) -> Self {
        Self {
            role: Arc::clone(&self.role),
        }
    }
}

/// A non-empty sequence of distinct application roles in semantic order.
#[derive(Debug, Eq, PartialEq)]
pub struct OrderedRoles<Role> {
    declared: Vec<Role>,
}

/// A repeated role and every role value retained by failed roster validation.
#[derive(Debug, Eq, PartialEq)]
pub struct DuplicateRole<Role> {
    /// Roles accepted before the repetition.
    pub declared: Vec<Role>,
    /// The repeated role.
    pub duplicate: Role,
    /// Roles not examined after the repetition.
    pub remaining: Vec<Role>,
}

impl<Role> OrderedRoles<Role>
where
    Role: Eq,
{
    /// Validate one required first role followed by any number of later roles.
    pub fn new(
        first: Role,
        remaining: impl IntoIterator<Item = Role>,
    ) -> Result<Self, DuplicateRole<Role>> {
        let mut declared = vec![first];
        let mut remaining = remaining.into_iter();
        loop {
            let role = match remaining.next() {
                Some(role) => role,
                None => return Ok(Self { declared }),
            };
            match declared.iter().position(|candidate| candidate == &role) {
                Some(_) => {
                    return Err(DuplicateRole {
                        declared,
                        duplicate: role,
                        remaining: remaining.collect(),
                    });
                }
                None => declared.push(role),
            }
        }
    }
}

impl<Role> OrderedRoles<Role> {
    pub(crate) fn into_roles(self) -> Vec<Role> {
        self.declared
    }
}
