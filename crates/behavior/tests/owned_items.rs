use behavior::{
    ActionItem, ActionItemResult, Here, InterpretSends, Interpretation, InterpretationProgress,
    InterpreterFault, InterpreterRequests, ItemSettlement, Never, SendSettlements, SettledItem,
    SourceCustody, SourceProgress, SourceSettlementCustody, settle_item,
};
use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};
use std::env::VarError;
use std::io::{self, Read};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Notify, oneshot};

#[path = "owned_items/requests.rs"]
mod requests;

use requests::{FileAccess, OpenFile, ReadEnvironment, Services};

#[tokio::test(flavor = "current_thread")]
#[expect(
    clippy::too_many_lines,
    reason = "one owned action lane proves suspension, producer disposal, exact pending custody, resumption and receipt-preserving replay together"
)]
async fn suspended_file_keeps_accepted_prefix_original_request_and_unattempted_suffix() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let expected_path = path.clone();
    let waiting_path = path.clone();
    let waiting_allocation = waiting_path.as_os_str().as_encoded_bytes().as_ptr();
    let rejected_path = path.join("missing");
    let rejected_allocation = rejected_path.as_os_str().as_encoded_bytes().as_ptr();
    let expected_rejected_path = rejected_path.clone();
    let entered = Arc::new(Notify::new());
    let (release, permission) = oneshot::channel();
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::new(
        vec![
            OpenFile {
                path,
                access: FileAccess::Ready,
            },
            OpenFile {
                path: waiting_path,
                access: FileAccess::Waiting {
                    entered: Arc::clone(&entered),
                    permission,
                },
            },
            OpenFile {
                path: rejected_path,
                access: FileAccess::Ready,
            },
        ],
    )));
    let mut services = Services;
    {
        let mut operation = pin!(<InterpreterRequests<OpenFile> as InterpretSends<
            Services,
            Never,
            Here,
        >>::interpret(&mut progress, &mut services,));
        let mut context = Context::from_waker(Waker::noop());
        let pending = operation.as_mut().poll(&mut context);
        assert!(matches!(pending, Poll::Pending));
    }
    let Some(InterpretationProgress::Interpreting(rows)) = &progress else {
        panic!("disposing the suspended producer preserves its caller-owned progress");
    };
    assert_eq!(rows.len(), 3);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(
        ItemSettlement::Accepted(file),
    ))) = &rows[0]
    else {
        panic!("the prefix file is already acquired");
    };
    let mut file = file;
    let mut prefix = [0_u8];
    file.read_exact(&mut prefix)
        .expect("advance the original acquired file before resuming");
    assert_eq!(
        prefix,
        [include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[0]]
    );
    let Some(InterpretationProgress::Interpreting((Some(waiting), None))) = &rows[1] else {
        panic!("the exact request remains owned while permission is pending");
    };
    assert_eq!(
        waiting.path.as_os_str().as_encoded_bytes().as_ptr(),
        waiting_allocation
    );
    assert_eq!(waiting.path, expected_path);
    let FileAccess::Waiting {
        entered: returned,
        permission: _,
    } = &waiting.access
    else {
        panic!("the pending request retains its actual permission capability");
    };
    assert!(Arc::ptr_eq(&entered, returned));
    let Some(InterpretationProgress::Original(suffix)) = &rows[2] else {
        panic!("the later request cannot be attempted before the pending request finishes");
    };
    assert_eq!(
        suffix.path.as_os_str().as_encoded_bytes().as_ptr(),
        rejected_allocation
    );
    assert_eq!(suffix.path, expected_rejected_path);
    assert!(matches!(suffix.access, FileAccess::Ready));
    let notified = entered.notified();
    let mut notified = pin!(notified);
    let mut context = Context::from_waker(Waker::noop());
    let started = notified.as_mut().poll(&mut context);
    assert!(matches!(started, Poll::Ready(())));
    let released = release.send(());
    released.expect("the original request still owns its permission receiver");
    <InterpreterRequests<OpenFile> as InterpretSends<Services, Never, Here>>::interpret(
        &mut progress,
        &mut services,
    )
    .await;
    <InterpreterRequests<OpenFile> as SendSettlements>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(items))) = &mut progress
    else {
        panic!("resumption completes the original file lane");
    };
    let SettledItem::Attempted(ItemSettlement::Accepted(file)) = &mut items[1] else {
        panic!("the late file was actually acquired");
    };
    let mut prefix = [0_u8];
    file.read_exact(&mut prefix)
        .expect("advance the late file before completed replay");
    assert_eq!(
        prefix,
        [include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[0]]
    );
    // Both advanced file offsets must survive the completed replay unchanged.
    <InterpreterRequests<OpenFile> as InterpretSends<Services, Never, Here>>::interpret(
        &mut progress,
        &mut services,
    )
    .await;
    let Some(InterpretationProgress::Completed(Interpretation::Complete(items))) = progress else {
        panic!("the resumed loan returns the complete original product");
    };
    assert_eq!(items.len(), 3);
    let mut items = items.into_iter();
    for _ in 0..2 {
        let Some(SettledItem::Attempted(ItemSettlement::Accepted(mut file))) = items.next() else {
            panic!("both the prefix and late file remain acquired");
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("the actual retained file remains usable");
        assert_eq!(
            contents,
            &include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[1..]
        );
    }
    let Some(SettledItem::Attempted(ItemSettlement::Rejected { item, reason })) = items.next()
    else {
        panic!("the later request is attempted after the late completion");
    };
    assert_eq!(
        item.path.as_os_str().as_encoded_bytes().as_ptr(),
        rejected_allocation
    );
    assert_eq!(item.path, expected_rejected_path);
    assert!(matches!(item.access, FileAccess::Ready));
    assert_eq!(reason.kind(), io::ErrorKind::NotADirectory);
}

#[tokio::test(flavor = "current_thread")]
async fn environment_receipts_and_original_rejections_share_one_complete_lane() {
    let missing = String::from("=");
    let allocation = missing.as_ptr();
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::new(
        vec![
            ReadEnvironment {
                name: String::from("CARGO_MANIFEST_DIR"),
            },
            ReadEnvironment { name: missing },
        ],
    )));
    let mut services = Services;
    <InterpreterRequests<ReadEnvironment> as InterpretSends<Services, Never, Here>>::interpret(
        &mut progress,
        &mut services,
    )
    .await;
    <InterpreterRequests<ReadEnvironment> as SendSettlements>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(items))) = progress else {
        panic!("both environment settlements remain in their original product");
    };
    assert_eq!(items.len(), 2);
    let mut items = items.into_iter();
    let Some(SettledItem::Attempted(ItemSettlement::Accepted(value))) = items.next() else {
        panic!("the actual environment value is acquired");
    };
    assert_eq!(value, env!("CARGO_MANIFEST_DIR"));
    let Some(SettledItem::Attempted(ItemSettlement::Rejected { item, reason })) = items.next()
    else {
        panic!("the complete original missing variable is returned");
    };
    assert_eq!(item.name.as_ptr(), allocation);
    assert_eq!(item.name, "=");
    assert_eq!(reason, VarError::NotPresent);
}

#[tokio::test(flavor = "current_thread")]
async fn closed_permission_rejects_the_complete_original_file_request() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let allocation = path.as_os_str().as_encoded_bytes().as_ptr();
    let expected_path = path.clone();
    let entered = Arc::new(Notify::new());
    let (release, permission) = oneshot::channel();
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::one(
        OpenFile {
            path,
            access: FileAccess::Waiting {
                entered: Arc::clone(&entered),
                permission,
            },
        },
    )));
    drop(release);
    let mut services = Services;
    <InterpreterRequests<OpenFile> as InterpretSends<Services, Never, Here>>::interpret(
        &mut progress,
        &mut services,
    )
    .await;
    <InterpreterRequests<OpenFile> as SendSettlements>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(items))) = progress else {
        panic!("closed permission is an ordinary complete rejection");
    };
    assert_eq!(items.len(), 1);
    let Some(SettledItem::Attempted(ItemSettlement::Rejected { item, reason })) =
        items.into_iter().next()
    else {
        panic!("failure cannot become a file receipt or discard its original request");
    };
    assert_eq!(
        item.path.as_os_str().as_encoded_bytes().as_ptr(),
        allocation
    );
    assert_eq!(item.path, expected_path);
    assert_eq!(reason.kind(), io::ErrorKind::Other);
    let FileAccess::Waiting {
        entered: returned,
        permission: _,
    } = item.access
    else {
        panic!("rejection preserves the request's permission capability");
    };
    assert!(Arc::ptr_eq(&entered, &returned));
}

#[tokio::test(flavor = "current_thread")]
async fn acquired_file_survives_source_reoffers_after_interpreter_disposal() {
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::one(
        OpenFile {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
            access: FileAccess::Ready,
        },
    )));
    {
        let mut services = Services;
        <InterpreterRequests<OpenFile> as InterpretSends<Services, Never, Here>>::interpret(
            &mut progress,
            &mut services,
        )
        .await;
    }
    <InterpreterRequests<OpenFile> as SendSettlements>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(mut receipts))) = progress
    else {
        panic!("the real file receipt outlives its interpreter");
    };
    let SettledItem::Attempted(ItemSettlement::Accepted(file)) = &mut receipts[0] else {
        panic!("opening the manifest acquired a file");
    };
    let mut prefix = [0_u8];
    file.read_exact(&mut prefix)
        .expect("advance the acquired file");
    assert_eq!(
        prefix,
        [include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[0]]
    );
    for _ in 0..2 {
        let mut source = Some(SourceProgress::Original(receipts));
        <Vec<ActionItemResult<OpenFile>> as SourceSettlementCustody<(), Never>>::prepare_source(
            &mut source,
        );
        let Some(SourceProgress::Offering(custody)) = &mut source else {
            panic!("the acquired file is offered for source settlement");
        };
        <Vec<ActionItemResult<OpenFile>> as SourceSettlementCustody<(), Never>>::offer_next_to_source(custody, &mut ()).await;
        <Vec<ActionItemResult<OpenFile>> as SourceSettlementCustody<(), Never>>::finish_source(
            &mut source,
        );
        let Some(SourceProgress::Completed(SourceCustody::Retained(retained))) = source else {
            panic!("the request's retention policy keeps its actual acquired file");
        };
        assert_eq!(retained.len(), 1);
        receipts = retained;
    }
    let Some(SettledItem::Attempted(ItemSettlement::Accepted(mut file))) = receipts.pop() else {
        panic!("the same file remains after repeated source offers");
    };
    let mut remaining = String::new();
    file.read_to_string(&mut remaining)
        .expect("read the retained file");
    assert_eq!(
        remaining,
        &include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[1..]
    );
    assert!(receipts.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn acquired_environment_survives_source_reoffers_after_interpreter_disposal() {
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::one(
        ReadEnvironment {
            name: String::from("CARGO_MANIFEST_DIR"),
        },
    )));
    {
        let mut services = Services;
        <InterpreterRequests<ReadEnvironment> as InterpretSends<Services, Never, Here>>::interpret(
            &mut progress,
            &mut services,
        )
        .await;
    }
    <InterpreterRequests<ReadEnvironment> as SendSettlements>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(mut receipts))) = progress
    else {
        panic!("the actual environment receipt outlives its interpreter");
    };
    let SettledItem::Attempted(ItemSettlement::Accepted(value)) = &receipts[0] else {
        panic!("the manifest directory was read from the environment");
    };
    let allocation = value.as_ptr();
    for _ in 0..2 {
        let mut source = Some(SourceProgress::Original(receipts));
        <Vec<ActionItemResult<ReadEnvironment>> as SourceSettlementCustody<(), Never>>::prepare_source(&mut source);
        let Some(SourceProgress::Offering(custody)) = &mut source else {
            panic!("the acquired environment value is offered for source settlement");
        };
        <Vec<ActionItemResult<ReadEnvironment>> as SourceSettlementCustody<(), Never>>::offer_next_to_source(custody, &mut ()).await;
        <Vec<ActionItemResult<ReadEnvironment>> as SourceSettlementCustody<(), Never>>::finish_source(&mut source);
        let Some(SourceProgress::Completed(SourceCustody::Retained(retained))) = source else {
            panic!("the request's retention policy keeps its actual environment value");
        };
        assert_eq!(retained.len(), 1);
        receipts = retained;
    }
    let Some(SettledItem::Attempted(ItemSettlement::Accepted(value))) = receipts.pop() else {
        panic!("the same environment value remains after repeated source offers");
    };
    assert_eq!(value.as_ptr(), allocation);
    assert_eq!(value, env!("CARGO_MANIFEST_DIR"));
    assert!(receipts.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn file_receipt_survives_panic_before_item_finalization() {
    let mut progress = Some(InterpretationProgress::Original(OpenFile {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        access: FileAccess::Ready,
    }));
    {
        let mut services = Services;
        let mut operation = pin!(async {
            settle_item::<OpenFile, Services, Never, Here>(&mut progress, &mut services).await;
            panic!("injected producer panic after acquiring the actual file receipt");
        });
        let mut context = Context::from_waker(Waker::noop());
        let result = catch_unwind(AssertUnwindSafe(|| operation.as_mut().poll(&mut context)));
        assert!(result.is_err());
    }
    let Some(InterpretationProgress::Interpreting((None, Some(ItemSettlement::Accepted(file))))) =
        &mut progress
    else {
        panic!("the outside destination keeps the file after the producer unwinds");
    };
    let mut prefix = [0_u8];
    file.read_exact(&mut prefix)
        .expect("advance the actual receipt before finalization");
    assert_eq!(
        prefix,
        [include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[0]]
    );
    <OpenFile as ActionItem>::finish_interpretation(&mut progress);
    <OpenFile as ActionItem>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(
        ItemSettlement::Accepted(mut file),
    ))) = progress
    else {
        panic!("repeated finalization preserves the complete accepted receipt");
    };
    let mut remaining = String::new();
    file.read_to_string(&mut remaining)
        .expect("the file remains usable after producer panic");
    assert_eq!(
        remaining,
        &include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[1..]
    );
}

#[tokio::test(flavor = "current_thread")]
#[expect(
    clippy::too_many_lines,
    reason = "one suspended real file lane proves the exact accepted prefix, injected corruption, unattempted suffix and completed replay together"
)]
async fn corrupt_reply_keeps_acquired_file_original_request_and_unattempted_suffix() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let waiting = manifest.clone();
    let waiting_allocation = waiting.as_os_str().as_encoded_bytes().as_ptr();
    let suffix = manifest.clone();
    let suffix_allocation = suffix.as_os_str().as_encoded_bytes().as_ptr();
    let entered = Arc::new(Notify::new());
    let (release, permission) = oneshot::channel();
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::new(
        vec![
            OpenFile {
                path: manifest.clone(),
                access: FileAccess::Ready,
            },
            OpenFile {
                path: waiting,
                access: FileAccess::Waiting {
                    entered: Arc::clone(&entered),
                    permission,
                },
            },
            OpenFile {
                path: suffix,
                access: FileAccess::Ready,
            },
        ],
    )));
    let mut services = Services;
    {
        let mut operation = pin!(<InterpreterRequests<OpenFile> as InterpretSends<
            Services,
            Never,
            Here,
        >>::interpret(&mut progress, &mut services));
        let mut context = Context::from_waker(Waker::noop());
        let pending = operation.as_mut().poll(&mut context);
        assert!(matches!(pending, Poll::Pending));
    }
    let Some(InterpretationProgress::Interpreting(rows)) = &mut progress else {
        panic!("the suspended lane still owns all three rows");
    };
    let Some(InterpretationProgress::Completed(Interpretation::Complete(
        ItemSettlement::Accepted(file),
    ))) = &mut rows[0]
    else {
        panic!("the prefix file was actually acquired");
    };
    let mut prefix = [0_u8];
    file.read_exact(&mut prefix)
        .expect("advance the accepted prefix before injecting a later fault");
    assert_eq!(
        prefix,
        [include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[0]]
    );
    let Some(InterpretationProgress::Interpreting((input @ Some(_), received @ None))) =
        &mut rows[1]
    else {
        panic!("the waiting request remains in its receiving loan");
    };
    let item = input
        .take()
        .expect("the exact pending original is available to reject");
    // Fault injection at the interpreter reply destination; an OS refusal stays Rejected.
    *received = Some(ItemSettlement::Corrupt {
        item,
        fault: InterpreterFault::CorruptTraversal,
    });
    <InterpreterRequests<OpenFile> as InterpretSends<Services, Never, Here>>::interpret(
        &mut progress,
        &mut services,
    )
    .await;
    <InterpreterRequests<OpenFile> as InterpretSends<Services, Never, Here>>::interpret(
        &mut progress,
        &mut services,
    )
    .await;
    <InterpreterRequests<OpenFile> as SendSettlements>::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Corrupt(items))) = progress else {
        panic!("the blanket finalizer preserves the exact corruption verdict");
    };
    let [
        SettledItem::Attempted(ItemSettlement::Accepted(mut file)),
        SettledItem::Attempted(ItemSettlement::Corrupt {
            item,
            fault: InterpreterFault::CorruptTraversal,
        }),
        SettledItem::Unattempted(suffix),
    ]: [ActionItemResult<OpenFile>; 3] = items
        .try_into()
        .unwrap_or_else(|_| panic!("every original row remains"))
    else {
        panic!("corruption retains the acquired prefix and prevents later interpretation");
    };
    let mut remaining = String::new();
    file.read_to_string(&mut remaining)
        .expect("the committed prefix keeps its advanced file");
    assert_eq!(
        remaining,
        &include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))[1..]
    );
    assert_eq!(item.path, manifest);
    assert_eq!(
        item.path.as_os_str().as_encoded_bytes().as_ptr(),
        waiting_allocation
    );
    let FileAccess::Waiting {
        entered: returned,
        permission: _,
    } = &item.access
    else {
        panic!("the corrupt original retains its exact permission capability");
    };
    assert!(Arc::ptr_eq(&entered, returned));
    let released = release.send(());
    released.expect("the corrupt original still owns its actual permission receiver");
    assert_eq!(suffix.path, manifest);
    assert_eq!(
        suffix.path.as_os_str().as_encoded_bytes().as_ptr(),
        suffix_allocation
    );
    assert!(matches!(suffix.access, FileAccess::Ready));
}
