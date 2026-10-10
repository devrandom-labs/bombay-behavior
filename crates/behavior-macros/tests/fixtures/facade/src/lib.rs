//! The facade library and its documentation share one canonical owner path.
//!
//! ```
//! struct Counter { value: u8 }
//! #[bombay::behavior::behavior(addr = bombay::behavior::MailAddr, message = u8)]
//! impl Counter {
//!     fn receive(&mut self, _: bombay::behavior::MailAddr, value: u8)
//!         -> bombay::behavior::BehaviorActed<Self>
//!     {
//!         self.value = value;
//!         Ok(bombay::behavior::Actions::cont())
//!     }
//! }
//! let mut counter = Counter { value: 0 };
//! let actions = bombay::behavior::delegate_transition(
//!     &mut counter,
//!     bombay::behavior::User::new(bombay::behavior::MailAddr(1), 4),
//! ).expect("the concrete counter transition succeeds");
//! assert_eq!(counter.value, 4);
//! assert_eq!(actions, bombay::behavior::Actions::cont());
//! ```
//!
//! ```
//! struct Worker;
//! #[bombay::atomic::pool_worker(addr = bombay::behavior::MailAddr, result = u16)]
//! impl Worker {
//!     fn transition(&mut self, assignment: bombay::atomic::Assignment<u8>)
//!         -> WorkerActed<Self>
//!     {
//!         Ok(bombay::behavior::Actions::cont().with_send(assignment.complete(7)))
//!     }
//! }
//! ```

pub use catalog::atomic;
pub use foundation as behavior;
