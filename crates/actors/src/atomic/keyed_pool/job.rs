//! Customer custody retained by accepted keyed work.

use super::BindingEvidence;

pub(super) struct KeyedCustomer<Role, Route> {
    pub(super) binding: BindingEvidence<Role>,
    pub(super) route: Route,
}
