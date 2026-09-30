use std::fmt;
use std::sync::{Arc, Mutex, mpsc};

use behavior::Behavior;

pub struct InstalledControl<B: Behavior, Endpoint> {
    endpoint: Endpoint,
    sender: mpsc::Sender<B::Event>,
    receiver: Arc<Mutex<mpsc::Receiver<B::Event>>>,
}

impl<B: Behavior, Endpoint: Clone> Clone for InstalledControl<B, Endpoint> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            sender: self.sender.clone(),
            receiver: Arc::clone(&self.receiver),
        }
    }
}

impl<B: Behavior, Endpoint> PartialEq for InstalledControl<B, Endpoint> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.receiver, &other.receiver)
    }
}

impl<B: Behavior, Endpoint> Eq for InstalledControl<B, Endpoint> {}

impl<B: Behavior, Endpoint: fmt::Debug> fmt::Debug for InstalledControl<B, Endpoint> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledControl")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl<B: Behavior, Endpoint> InstalledControl<B, Endpoint> {
    #[allow(
        dead_code,
        reason = "some interpreter fixtures only reject child creation"
    )]
    pub fn new(endpoint: Endpoint) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            endpoint,
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}
