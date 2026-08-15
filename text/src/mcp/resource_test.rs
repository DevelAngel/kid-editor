//! Test scaffolding for verifying whether a connected MCP client reacts
//! to `notifications/resources/updated`. Exposes one dummy resource
//! (`test://counter`) whose text content embeds a monotonically
//! increasing counter — the changing content defeats client-side
//! caching, so a stale re-read is distinguishable from a real one.
//!
//! One tool, `test_timer_control`, starts or stops a background ticker
//! that increments the counter and pushes the notification on each
//! tick. No polling loop is exposed to the client directly; the timer
//! only runs while explicitly started, and every tick, subscribe, and
//! read is logged so the two questions this exists to answer —
//! does the client subscribe, and does it re-read after being notified
//! — are visible in the server log.

use super::McpService;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, ErrorData as McpError, ReadResourceResult, Resource,
    ResourceContents, ResourceUpdatedNotificationParam,
};
use rmcp::schemars::{self, JsonSchema};
use rmcp::service::Peer;
use rmcp::{RoleServer, tool, tool_router};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time;
use tokio_util::sync::CancellationToken;

use std::sync::Arc;
use std::time::Duration;

/// URI of the one dummy resource this module exposes.
pub(super) const COUNTER_URI: &str = "test://counter";

/// Shared, per-session state: the counter itself, the running ticker's
/// cancellation handle (if any), and the peer to notify (set on
/// `subscribe`, cleared on `unsubscribe`). One `McpService` per
/// session (see `McpServer::serve`'s session factory), so this never
/// needs to distinguish between multiple clients.
#[derive(Default)]
pub(super) struct CounterState {
    count: u64,
    cancel: Option<CancellationToken>,
    peer: Option<Peer<RoleServer>>,
}

pub(super) type SharedCounterState = Arc<Mutex<CounterState>>;

/// Current counter value, for `read_resource`.
pub(super) async fn read(state: &SharedCounterState) -> u64 {
    state.lock().await.count
}

/// Records the subscribing peer so ticks have someone to notify.
pub(super) async fn subscribe(state: &SharedCounterState, peer: Peer<RoleServer>) {
    state.lock().await.peer = Some(peer);
}

/// Drops the peer — no more notifications until the client resubscribes.
pub(super) async fn unsubscribe(state: &SharedCounterState) {
    state.lock().await.peer = None;
}

/// One resource: the dummy counter. A real deployment would list one
/// entry per watched thing; here there is exactly one to test with.
pub(super) fn resource() -> Resource {
    Resource::new(COUNTER_URI, "Notification Test Counter")
}

pub(super) fn read_result(count: u64) -> ReadResourceResult {
    ReadResourceResult::new(vec![ResourceContents::text(
        format!("counter: {count}"),
        COUNTER_URI,
    )])
}

pub(super) fn is_counter(uri: &str) -> bool {
    uri == COUNTER_URI
}

/// `Stop`/`Start { interval_secs }` as one enum instead of two tools —
/// a single control surface for the one thing being toggled. Nested
/// inside `TimerControlInput` rather than used directly as the tool's
/// root parameter: an internally tagged enum's own schema is a `oneOf`
/// with no root `type: object`, which the MCP spec requires of a
/// tool's input schema. Nesting it as a field sidesteps that — the
/// root schema is `TimerControlInput`'s (an object), not the enum's.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action")]
enum TimerCommand {
    Stop,
    Start { interval_secs: u64 },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TimerControlInput {
    command: TimerCommand,
}

#[tool_router(router = resource_test_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Start or stop a background timer that increments a counter and pushes a `notifications/resources/updated` for the `test://counter` resource on every tick. Used to test whether the connected client reacts to resource-update notifications by re-reading the resource.",
        annotations(
            title = "Resource Notification Test Timer",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn test_timer_control(
        &self,
        Parameters(input): Parameters<TimerControlInput>,
    ) -> Result<CallToolResult, McpError> {
        let mut state = self.counter.lock().await;
        if let Some(cancel) = state.cancel.take() {
            cancel.cancel();
        }

        let message = match input.command {
            TimerCommand::Stop => "timer stopped".to_owned(),
            TimerCommand::Start { interval_secs } => {
                let cancel = CancellationToken::new();
                state.cancel = Some(cancel.clone());
                spawn_ticker(
                    self.counter.clone(),
                    Duration::from_secs(interval_secs),
                    cancel,
                );
                format!("timer started, interval {interval_secs}s")
            }
        };
        drop(state);

        tracing::info!(%message, "test_timer_control");
        Ok(CallToolResult::success(vec![ContentBlock::text(message)]))
    }
}

/// Runs until `cancel` fires. Each tick increments the counter and, if
/// a client is currently subscribed, sends the update notification.
/// Silently skips the notification (just logs) when nobody is
/// subscribed yet — that's expected before the first `subscribe`.
fn spawn_ticker(state: SharedCounterState, interval: Duration, cancel: CancellationToken) {
    tokio::spawn(async move {
        let mut ticker = time::interval(interval);
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    tracing::debug!("counter ticker cancelled");
                    break;
                }
                _ = ticker.tick() => {
                    let peer = {
                        let mut guard = state.lock().await;
                        guard.count += 1;
                        tracing::debug!(count = guard.count, "counter tick");
                        guard.peer.clone()
                    };
                    let Some(peer) = peer else {
                        tracing::debug!("no subscriber yet, skipping notification");
                        continue;
                    };
                    match peer
                        .notify_resource_updated(ResourceUpdatedNotificationParam::new(
                            COUNTER_URI,
                        ))
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(uri = COUNTER_URI, "sent resources/updated notification");
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to send resources/updated notification");
                        }
                    }
                }
            }
        }
    });
}
