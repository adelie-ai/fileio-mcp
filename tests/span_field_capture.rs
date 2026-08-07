#![deny(warnings)]

//! Span-field acceptance test (mcp-core#40).
//!
//! `telemetry_suite.rs` reads the console text a real process writes to
//! stderr. That misses one class of leak: `#[tracing::instrument]` captures
//! a function's arguments into the span's *fields* the moment the span is
//! created, whether or not any event ever renders that span on the console.
//! With the `otel` feature and a collector attached, a span exports on close
//! with every field it holds, independent of local console output. A
//! console-only test cannot see that path, so this one drives the dispatch
//! in process under a capturing `tracing` layer and reads the raw spans and
//! events back, the same way mcp-core's own telemetry tests do.
//!
//! This test is table-driven over every registered tool
//! ([`support::sentinel_tool_calls`]), not just one. An earlier version
//! called only `fileio_read_lines`; review proved that dropping `skip_all`
//! from `write_file.rs` or `cp.rs` passed both the console test and that
//! single-tool version of this one. `support::assert_table_covers_every_tool`
//! fails the test outright if a tool is registered without a matching
//! table entry, so the table cannot silently fall behind the dispatch again.

mod support;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use fileio_mcp::path_guard::PathGuard;
use fileio_mcp::service::FileIoService;
use mcp_core::{ServerCore, Session};
use serde_json::json;
use support::{assert_table_covers_every_tool, sentinel_tool_calls};
use tempfile::TempDir;
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

/// One recorded span or event: its name, its level, and its fields as the
/// subscriber saw them.
#[derive(Clone, Debug)]
struct Recorded {
    name: &'static str,
    level: Level,
    fields: BTreeMap<String, String>,
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Recorded>>>);

impl Capture {
    fn take(self) -> Vec<Recorded> {
        self.0.lock().expect("capture lock").clone()
    }
}

impl<S> Layer<S> for Capture
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        attrs.record(&mut Collector(&mut fields));
        self.0.lock().expect("capture lock").push(Recorded {
            name: attrs.metadata().name(),
            level: *attrs.metadata().level(),
            fields,
        });
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        event.record(&mut Collector(&mut fields));
        self.0.lock().expect("capture lock").push(Recorded {
            name: "<event>",
            level: *event.metadata().level(),
            fields,
        });
    }
}

struct Collector<'a>(&'a mut BTreeMap<String, String>);

impl Visit for Collector<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

/// Run `body` with a capturing subscriber installed for this thread, and
/// return everything it recorded. A dedicated current-thread runtime, not
/// `#[tokio::test]`: `tracing::subscriber::with_default` scopes to the
/// calling thread, and a multi-threaded runtime could hop the dispatch onto
/// a thread the capture never sees.
fn capture<F, Fut>(body: F) -> Vec<Recorded>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()>,
{
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    tracing::subscriber::with_default(subscriber, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a current-thread runtime");
        runtime.block_on(body());
    });
    capture.take()
}

/// Acceptance (mcp-core#40, D10): a span field is not naturally leveled the
/// way an event is filtered by `RUST_LOG`, so `#[tracing::instrument]`'s
/// implicit argument capture is exactly the leak a console-only test cannot
/// see. Drives every registered tool once, with allowed (never denied)
/// sentinel-bearing arguments so each tool's own operation function actually
/// runs and opens its span - a denied call returns before that function is
/// reached at all, which is the wrong path to prove this on. Checks every
/// recorded span and event whose level is INFO or stricter (never
/// DEBUG/TRACE, where a tool argument is allowed) for its tool's sentinel,
/// in a field value rather than only in a rendered message, with a
/// per-tool DEBUG-level positive control.
#[test]
fn no_span_or_info_level_event_field_carries_a_path() {
    let root = TempDir::new().expect("create temp root");
    let calls = sentinel_tool_calls(root.path());
    assert_table_covers_every_tool(&calls);

    let service = FileIoService::with_guard(PathGuard::default());
    let core = ServerCore::new(fileio_mcp::server_config(), Arc::new(service));

    // Build the JSON-RPC requests up front so the closure below can own
    // them and `calls` stays available afterward for the assertions.
    let mut requests = vec![json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-11-25", "capabilities": {}},
    })];
    for (offset, call) in calls.iter().enumerate() {
        requests.push(json!({
            "jsonrpc": "2.0",
            "id": 2 + offset as u64,
            "method": "tools/call",
            "params": {"name": call.name, "arguments": call.arguments},
        }));
    }

    let recorded = capture(|| async move {
        // Route through the real dispatch path (Session::handle_message),
        // not FileIoService::call_tool directly - the DEBUG-level "tool
        // call arguments" event this test's positive control depends on
        // lives in mcp-core's Session, one layer up from the service.
        let mut session = Session::new(core);
        for request in requests {
            session.handle_message(request).await;
        }
    });

    let mut leaks = Vec::new();
    let mut seen_below_info: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for item in &recorded {
        for call in &calls {
            if !call.carries_sentinel {
                continue;
            }
            let sentinel = support::sentinel_for(call.name);
            let carries = item.fields.values().any(|v| v.contains(&sentinel));
            if !carries {
                continue;
            }
            if item.level == Level::DEBUG || item.level == Level::TRACE {
                seen_below_info.insert(call.name);
            } else {
                leaks.push(format!(
                    "tool {} -> {} [{}] {:?}",
                    call.name, item.name, item.level, item.fields
                ));
            }
        }
    }

    assert!(
        leaks.is_empty(),
        "a path reached a span or event field at INFO or stricter:\n{}",
        leaks.join("\n")
    );

    let expected_controls: Vec<&str> = calls
        .iter()
        .filter(|c| c.carries_sentinel)
        .map(|c| c.name)
        .collect();
    let missing_controls: Vec<&&str> = expected_controls
        .iter()
        .filter(|name| !seen_below_info.contains(*name))
        .collect();
    assert!(
        missing_controls.is_empty(),
        "expected every content-bearing tool's sentinel to appear at DEBUG or \
         TRACE as a positive control (mcp-core logs tool arguments at DEBUG \
         per D10); these tools' sentinels never appeared anywhere, so the \
         absence check above is not meaningful for them: {missing_controls:?}"
    );
}
