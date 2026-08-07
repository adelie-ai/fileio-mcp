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

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use fileio_mcp::path_guard::PathGuard;
use fileio_mcp::service::FileIoService;
use mcp_core::{ServerCore, Session};
use serde_json::json;
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

/// The value that must never reach a span field or an event field at DEBUG's
/// stricter neighbor levels (INFO/WARN/ERROR) — content, not an id or a
/// count.
const SENTINEL: &str = "SENTINEL-SPAN-FIELD-CAPTURE-7a91";

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
/// see. This drives one denied and one allowed call, both against
/// sentinel-bearing paths, and checks every recorded span and event whose
/// level is INFO or stricter (never DEBUG/TRACE, where a tool argument is
/// allowed) for the sentinel — in a field value, not only in a rendered
/// message.
#[test]
fn no_span_or_info_level_event_field_carries_a_path() {
    let allowed_path = format!("/tmp/{SENTINEL}-allowed/does-not-need-to-exist.txt");
    let denied_path = format!("/tmp/{SENTINEL}-denied/secret.txt");

    let guard = PathGuard::new(&[format!("/tmp/{SENTINEL}-denied/")], None);
    let service = FileIoService::with_guard(guard);
    let core = ServerCore::new(fileio_mcp::server_config(), Arc::new(service));

    let recorded = capture(|| async move {
        // Route through the real dispatch path (Session::handle_message),
        // not FileIoService::call_tool directly — the DEBUG-level "tool
        // call arguments" event this test's positive control depends on
        // lives in mcp-core's Session, one layer up from the service.
        let mut session = Session::new(core);
        session
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-11-25", "capabilities": {}},
            }))
            .await;
        session
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": "fileio_read_lines", "arguments": {"path": allowed_path}},
            }))
            .await;
        session
            .handle_message(json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "fileio_read_lines", "arguments": {"path": denied_path}},
            }))
            .await;
    });

    let mut saw_sentinel_below_info = false;
    let mut leaks = Vec::new();
    for item in &recorded {
        let carries_sentinel = item.fields.values().any(|v| v.contains(SENTINEL));
        if !carries_sentinel {
            continue;
        }
        if item.level == Level::DEBUG || item.level == Level::TRACE {
            saw_sentinel_below_info = true;
            continue;
        }
        leaks.push(format!("{} [{}] {:?}", item.name, item.level, item.fields));
    }

    assert!(
        leaks.is_empty(),
        "a path reached a span or event field at INFO or stricter:\n{}",
        leaks.join("\n")
    );
    assert!(
        saw_sentinel_below_info,
        "expected the sentinel to appear at DEBUG or TRACE as a positive control \
         (mcp-core logs tool arguments at DEBUG per D10); seeing none means this \
         test captured nothing and the assertion above is not meaningful"
    );
}
