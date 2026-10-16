use behavior::{
    ActionItem, InterpretItem, InterpreterRequest, ItemSettlement, Never, NoBirthProtocols,
    NoReturnToEmitter, OwnedItem,
};
use core::future::Future;
use std::env::{self, VarError};
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Notify, oneshot};

pub(super) struct OpenFile {
    pub(super) path: PathBuf,
    pub(super) access: FileAccess,
}

pub(super) enum FileAccess {
    Ready,
    Waiting {
        entered: Arc<Notify>,
        permission: oneshot::Receiver<()>,
    },
}

pub(super) struct ReadEnvironment {
    pub(super) name: String,
}

pub(super) struct Services;

impl InterpreterRequest for OpenFile {
    type ReturnToEmitter = NoReturnToEmitter;
    type LogicalProtocols = NoBirthProtocols;
}

impl InterpreterRequest for ReadEnvironment {
    type ReturnToEmitter = NoReturnToEmitter;
    type LogicalProtocols = NoBirthProtocols;
}

impl OwnedItem for OpenFile {
    type Accepted = File;
    type Rejection = io::Error;
    type Prerequisite = Never;

    fn retain_accepted(file: File) -> Option<File> {
        Some(file)
    }
}

impl OwnedItem for ReadEnvironment {
    type Accepted = String;
    type Rejection = VarError;
    type Prerequisite = Never;

    fn retain_accepted(value: String) -> Option<String> {
        Some(value)
    }
}

impl<Event, Path> InterpretItem<OpenFile, Event, Path> for Services {
    #[expect(
        clippy::manual_async_fn,
        reason = "async fn captures Event and Path lifetimes that the receiving-loan contract does not require"
    )]
    fn interpret_item<'a>(
        &'a mut self,
        input: <OpenFile as ActionItem>::Input<'a>,
        received: &'a mut Option<<OpenFile as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        OpenFile: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(request) = input.as_mut() else {
                return;
            };
            let permission = match &mut request.access {
                FileAccess::Ready => Ok(()),
                FileAccess::Waiting {
                    entered,
                    permission,
                } => {
                    entered.notify_one();
                    permission.await.map_err(io::Error::other)
                }
            };
            let Some(request) = input.take() else {
                return;
            };
            *received = Some(match permission.and_then(|()| File::open(&request.path)) {
                Ok(file) => ItemSettlement::Accepted(file),
                Err(reason) => ItemSettlement::Rejected {
                    item: request,
                    reason,
                },
            });
        }
    }
}

impl<Event, Path> InterpretItem<ReadEnvironment, Event, Path> for Services {
    #[expect(
        clippy::manual_async_fn,
        reason = "async fn captures Event and Path lifetimes that the receiving-loan contract does not require"
    )]
    fn interpret_item<'a>(
        &'a mut self,
        input: <ReadEnvironment as ActionItem>::Input<'a>,
        received: &'a mut Option<<ReadEnvironment as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ReadEnvironment: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(request) = input.take() else {
                return;
            };
            *received = Some(match env::var(&request.name) {
                Ok(value) => ItemSettlement::Accepted(value),
                Err(reason) => ItemSettlement::Rejected {
                    item: request,
                    reason,
                },
            });
        }
    }
}
