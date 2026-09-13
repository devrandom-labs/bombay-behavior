//! Independent reference models shared by adversarial tests.

/// Independent activity-history model for one-notification-per-idle-period
/// receive timeout. Service traffic is deliberately absent from `activity`:
/// it never changes the expected live token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InactivityModel {
    last_token: Option<u64>,
    live_token: Option<u64>,
}

impl InactivityModel {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_token: None,
            live_token: None,
        }
    }

    /// Successful continuing initialization begins the first idle period.
    pub fn initialize(&mut self) -> u64 {
        self.arm()
    }

    /// A successful continuing user communication begins a new idle period.
    pub fn activity(&mut self) -> Option<u64> {
        self.last_token
            .and_then(|token| token.checked_add(1))
            .inspect(|&token| {
                self.last_token = Some(token);
                self.live_token = Some(token);
            })
    }

    /// Errors, terminal turns, and all service traffic leave timer state alone.
    #[must_use]
    pub const fn no_activity(&self) -> Option<u64> {
        self.live_token
    }

    /// Consume only the token for the current idle period.
    pub fn notification(&mut self, token: u64) -> bool {
        if self.live_token == Some(token) {
            self.live_token = None;
            true
        } else {
            false
        }
    }

    fn arm(&mut self) -> u64 {
        self.last_token = Some(0);
        self.live_token = Some(0);
        0
    }
}

impl Default for InactivityModel {
    fn default() -> Self {
        Self::new()
    }
}
