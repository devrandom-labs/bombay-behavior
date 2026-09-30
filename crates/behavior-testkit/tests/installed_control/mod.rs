use std::sync::{Arc, Mutex, mpsc};

use behavior_core::Behavior;

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

impl<B: Behavior, Endpoint> InstalledControl<B, Endpoint> {
    #[allow(dead_code, reason = "rejection-only fixtures do not commit a child")]
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
