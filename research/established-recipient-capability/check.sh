#!/usr/bin/env bash
set -euo pipefail

campaign_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
probe_dir="$campaign_dir/probe"

cargo test --manifest-path "$probe_dir/Cargo.toml" --locked --all-targets

if output=$(cargo check --manifest-path "$probe_dir/Cargo.toml" --locked --bin e0207 --features e0207 2>&1); then
    echo "expected the unconstrained endpoint probe to fail" >&2
    exit 1
fi
case "$output" in
    *E0207*) ;;
    *)
        echo "$output" >&2
        echo "unconstrained endpoint probe failed for the wrong reason" >&2
        exit 1
        ;;
esac

if output=$(cargo check --manifest-path "$probe_dir/Cargo.toml" --locked --bin wrong-protocol --features wrong-protocol 2>&1); then
    echo "expected the cross-protocol recipient probe to fail" >&2
    exit 1
fi
case "$output" in
    *'expected `EstablishedRecipient<Queue>`'*'found `EstablishedRecipient<Worker>`'*) ;;
    *)
        echo "$output" >&2
        echo "cross-protocol recipient probe failed for the wrong reason" >&2
        exit 1
        ;;
esac

if output=$(cargo check --manifest-path "$probe_dir/Cargo.toml" --locked --bin direct-use --features direct-use 2>&1); then
    echo "expected direct endpoint extraction to fail" >&2
    exit 1
fi
case "$output" in
    *'no method named `into_endpoint` found'*) ;;
    *)
        echo "$output" >&2
        echo "direct endpoint extraction failed for the wrong reason" >&2
        exit 1
        ;;
esac

if output=$(cargo check --manifest-path "$probe_dir/Cargo.toml" --locked --bin wrong-role --features wrong-role 2>&1); then
    echo "expected the cross-role creation fact probe to fail" >&2
    exit 1
fi
case "$output" in
    *'expected `CreationResolved<Parent, SecondaryRole>`'*'found `CreationResolved<Parent, PrimaryRole>`'*) ;;
    *)
        echo "$output" >&2
        echo "cross-role creation fact probe failed for the wrong reason" >&2
        exit 1
        ;;
esac

if output=$(cargo check --manifest-path "$probe_dir/Cargo.toml" --locked --bin rejected-capability --features rejected-capability 2>&1); then
    echo "expected rejected creation capability access to fail" >&2
    exit 1
fi
case "$output" in
    *'no field `recipient` on type `CreationResolved<Parent, PrimaryRole>`'*) ;;
    *)
        echo "$output" >&2
        echo "rejected creation capability access failed for the wrong reason" >&2
        exit 1
        ;;
esac

if output=$(cargo check --manifest-path "$probe_dir/Cargo.toml" --locked --bin staged-delivery --features staged-delivery 2>&1); then
    echo "expected staged-child established delivery to fail" >&2
    exit 1
fi
case "$output" in
    *'expected `EstablishedRecipient<_>`'*'found `StagedChild<Parent, PrimaryRole>`'*) ;;
    *)
        echo "$output" >&2
        echo "staged-child established delivery failed for the wrong reason" >&2
        exit 1
        ;;
esac

if rg -n '\b(dyn|Any|TypeId|unsafe|Box|HashMap)\b' "$campaign_dir/probe/src"; then
    echo "forbidden representation found in probe" >&2
    exit 1
fi

echo "established-recipient capability probe passed"
