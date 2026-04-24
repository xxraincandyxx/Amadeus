// @amadeus-header
// summary: Shared interactive session bridge for in-process and remote clients.
// layer: core
// status: active
// feature_flags:
// - api
// - tui
// provides:
// - module: crate::bridge
// - type: crate::bridge::BridgeSessionStatus
// - type: crate::bridge::BridgeSessionInfo
// - type: crate::bridge::BridgeEvent
// - type: crate::bridge::LocalSessionBridge
// uses:
// - module: crate::agent
// - module: crate::client
// - module: crate::error
// - runtime: tokio async runtime
// - protocol: serde serialization
// invariants:
// - Session state and event streams stay aligned across bridge consumers.
// side_effects:
// - Spawns asynchronous tasks.
// - Sends or receives messages across async channels.
// tests:
// - cmd: cargo test -p core bridge --features full
// @end-amadeus-header

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::agent::loop_agent::create_approval_channels;
use crate::agent::{
    Agent, AgentEvent, AgentProfile, ApprovalDecision, ApprovalRequest, Config, RunResult,
    SessionCheckpoint,
};
use crate::client::LLMClient;
use crate::error::{AgentError, Result};

const SESSION_EVENT_BUFFER: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeSessionStatus {
    Idle,
    Running,
    AwaitingApproval,
    Completed,
    Failed,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeSessionInfo {
    pub id: String,
    pub name: String,
    pub profile: String,
    pub status: BridgeSessionStatus,
    pub parent_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeEvent {
    SessionCreated {
        session: BridgeSessionInfo,
    },
    SessionUpdated {
        session: BridgeSessionInfo,
    },
    Agent {
        session_id: String,
        event: AgentEvent,
    },
    ChildSessionSpawned {
        parent_session_id: String,
        request_id: String,
        prompt: String,
        depth: usize,
        session: BridgeSessionInfo,
    },
}

struct BridgeSession<C: LLMClient> {
    info: BridgeSessionInfo,
    agent: Agent<C>,
    events_tx: broadcast::Sender<BridgeEvent>,
    pending_approvals: HashMap<String, ApprovalRequest>,
    approval_tx: Option<mpsc::Sender<(String, ApprovalDecision)>>,
    task: Option<JoinHandle<()>>,
    parent_request_id: Option<String>,
}

impl<C: LLMClient> BridgeSession<C> {
    fn new(
        info: BridgeSessionInfo,
        agent: Agent<C>,
        parent_request_id: Option<String>,
    ) -> (Self, broadcast::Receiver<BridgeEvent>) {
        let (events_tx, events_rx) = broadcast::channel(SESSION_EVENT_BUFFER);
        (
            Self {
                info,
                agent,
                events_tx,
                pending_approvals: HashMap::new(),
                approval_tx: None,
                task: None,
                parent_request_id,
            },
            events_rx,
        )
    }
}

type BridgeSessionHandle<C> = Arc<Mutex<BridgeSession<C>>>;
type BridgeSessionMap<C> = HashMap<String, BridgeSessionHandle<C>>;

#[derive(Clone)]
pub struct LocalSessionBridge<C: LLMClient + Clone + 'static> {
    client: C,
    config: Arc<Config>,
    sessions: Arc<RwLock<BridgeSessionMap<C>>>,
    active_session_id: Arc<RwLock<Option<String>>>,
}

impl<C: LLMClient + Clone + 'static> LocalSessionBridge<C> {
    /// Create a new in-process session bridge.
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            active_session_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new root session and return its metadata.
    pub async fn create_session(
        &self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<BridgeSessionInfo> {
        self.create_session_with_agent(None, None, name, profile, None, None)
            .await
    }

    async fn create_session_with_agent(
        &self,
        id: Option<String>,
        parent_session_id: Option<String>,
        name: Option<String>,
        profile: AgentProfile,
        parent_request_id: Option<String>,
        agent: Option<Agent<C>>,
    ) -> Result<BridgeSessionInfo> {
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let name = name.unwrap_or_else(|| format!("session-{}", &id[..8]));
        let info = BridgeSessionInfo {
            id: id.clone(),
            name,
            profile: profile.to_string(),
            status: BridgeSessionStatus::Idle,
            parent_session_id,
        };

        let agent = match agent {
            Some(agent) => agent,
            None => Agent::builder(self.client.clone(), Arc::clone(&self.config))
                .with_default_tools()
                .build(),
        };

        let (session, mut events_rx) = BridgeSession::new(info.clone(), agent, parent_request_id);
        let session = Arc::new(Mutex::new(session));
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(id.clone(), Arc::clone(&session));
        }

        if self.active_session_id.read().await.is_none() {
            *self.active_session_id.write().await = Some(id);
        }

        {
            let session = session.lock().await;
            let _ = session.events_tx.send(BridgeEvent::SessionCreated {
                session: info.clone(),
            });
        }

        while events_rx.try_recv().is_ok() {}

        Ok(info)
    }

    /// List all known sessions.
    pub async fn list_sessions(&self) -> Vec<BridgeSessionInfo> {
        let sessions = self.sessions.read().await;
        let handles: Vec<_> = sessions.values().cloned().collect();
        drop(sessions);

        let mut infos = Vec::with_capacity(handles.len());
        for handle in handles {
            infos.push(handle.lock().await.info.clone());
        }
        infos.sort_by(|left, right| left.name.cmp(&right.name));
        infos
    }

    /// Look up one session by identifier.
    pub async fn get_session(&self, session_id: &str) -> Option<BridgeSessionInfo> {
        let session = self.session_handle(session_id).await?;
        let info = session.lock().await.info.clone();
        Some(info)
    }

    /// Return the active session identifier if one is selected.
    pub async fn active_session_id(&self) -> Option<String> {
        self.active_session_id.read().await.clone()
    }

    /// Set the active session identifier.
    pub async fn set_active_session(&self, session_id: &str) -> Result<()> {
        if self.session_handle(session_id).await.is_none() {
            return Err(AgentError::InvalidResponse(format!(
                "Session '{}' not found",
                session_id
            )));
        }
        *self.active_session_id.write().await = Some(session_id.to_string());
        self.emit_session_update(session_id).await?;
        Ok(())
    }

    /// Subscribe to bridge events for a specific session.
    pub async fn subscribe(&self, session_id: &str) -> Result<broadcast::Receiver<BridgeEvent>> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        let session = session.lock().await;
        Ok(session.events_tx.subscribe())
    }

    /// Submit a user input to an idle session and start streaming events.
    pub async fn submit_input(&self, session_id: &str, prompt: String) -> Result<()> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;

        let (agent, mut stream, events_tx, parent_session_id, parent_request_id) = {
            let mut session = session.lock().await;
            match session.info.status {
                BridgeSessionStatus::Running | BridgeSessionStatus::AwaitingApproval => {
                    return Err(AgentError::InvalidResponse(format!(
                        "Session '{}' is already running",
                        session_id
                    )))
                }
                BridgeSessionStatus::Closed => {
                    return Err(AgentError::InvalidResponse(format!(
                        "Session '{}' is closed",
                        session_id
                    )))
                }
                _ => {}
            }

            let (channels, handle) = create_approval_channels();
            session.info.status = BridgeSessionStatus::Running;
            session.pending_approvals.clear();
            session.approval_tx = Some(handle.decision_tx.clone());
            let stream = session.agent.run_stream_with_approval(Some(channels));
            (
                session.agent.clone(),
                stream,
                session.events_tx.clone(),
                session.info.parent_session_id.clone(),
                session.parent_request_id.clone(),
            )
        };

        agent
            .history()
            .write()
            .await
            .push(crate::agent::Message::user(&prompt));

        self.emit_session_update(session_id).await?;

        let bridge = self.clone();
        let session_id = session_id.to_string();
        let task_session_id = session_id.clone();
        let task = tokio::spawn(async move {
            while let Some(next) = futures::StreamExt::next(&mut stream).await {
                match next {
                    Ok(event) => {
                        bridge
                            .handle_agent_event(
                                &task_session_id,
                                &agent,
                                parent_session_id.as_deref(),
                                parent_request_id.as_deref(),
                                &event,
                            )
                            .await;
                        let _ = events_tx.send(BridgeEvent::Agent {
                            session_id: task_session_id.clone(),
                            event,
                        });
                    }
                    Err(error) => {
                        bridge
                            .fail_session(&task_session_id, error.to_string())
                            .await;
                        let _ = events_tx.send(BridgeEvent::Agent {
                            session_id: task_session_id.clone(),
                            event: AgentEvent::Error {
                                message: error.to_string(),
                            },
                        });
                        break;
                    }
                }
            }
        });

        let session = self.session_handle(&session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        session.lock().await.task = Some(task);
        Ok(())
    }

    /// Submit an approval decision for a pending approval in a session.
    pub async fn submit_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        decision: ApprovalDecision,
    ) -> Result<()> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        let approval_tx = {
            let mut session = session.lock().await;
            if !session.pending_approvals.contains_key(approval_id) {
                return Err(AgentError::InvalidResponse(format!(
                    "Approval '{}' not found for session '{}'",
                    approval_id, session_id
                )));
            }
            session.pending_approvals.remove(approval_id);
            if session.pending_approvals.is_empty() {
                session.info.status = BridgeSessionStatus::Running;
            }
            session.approval_tx.clone()
        };

        self.emit_session_update(session_id).await?;

        let approval_tx = approval_tx.ok_or_else(|| {
            AgentError::InvalidResponse(format!(
                "Session '{}' has no active approval channel",
                session_id
            ))
        })?;
        approval_tx
            .send((approval_id.to_string(), decision))
            .await
            .map_err(|_| {
                AgentError::InvalidResponse(format!(
                    "Approval channel closed for session '{}'",
                    session_id
                ))
            })?;
        Ok(())
    }

    /// Capture a checkpoint for the current session state.
    pub async fn checkpoint(&self, session_id: &str) -> Result<SessionCheckpoint> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        let checkpoint = session.lock().await.agent.checkpoint().await?;
        Ok(checkpoint)
    }

    /// Restore a checkpoint into an existing session.
    pub async fn restore_checkpoint(
        &self,
        session_id: &str,
        checkpoint: &SessionCheckpoint,
    ) -> Result<()> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        session
            .lock()
            .await
            .agent
            .restore_checkpoint(checkpoint)
            .await?;
        self.emit_session_update(session_id).await
    }

    /// Close a session and abort any in-flight work.
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        {
            let mut session = session.lock().await;
            if let Some(task) = session.task.take() {
                task.abort();
            }
            session.pending_approvals.clear();
            session.approval_tx = None;
            session.info.status = BridgeSessionStatus::Closed;
        }
        self.emit_session_update(session_id).await?;

        let active = self.active_session_id().await;
        if active.as_deref() == Some(session_id) {
            let next_active = self
                .list_sessions()
                .await
                .into_iter()
                .find(|session| {
                    session.id != session_id && session.status != BridgeSessionStatus::Closed
                })
                .map(|session| session.id);
            *self.active_session_id.write().await = next_active;
        }

        Ok(())
    }

    async fn session_handle(&self, session_id: &str) -> Option<BridgeSessionHandle<C>> {
        self.sessions.read().await.get(session_id).cloned()
    }

    async fn emit_session_update(&self, session_id: &str) -> Result<()> {
        let session = self.session_handle(session_id).await.ok_or_else(|| {
            AgentError::InvalidResponse(format!("Session '{}' not found", session_id))
        })?;
        let session = session.lock().await;
        let _ = session.events_tx.send(BridgeEvent::SessionUpdated {
            session: session.info.clone(),
        });
        Ok(())
    }

    async fn fail_session(&self, session_id: &str, message: String) {
        if let Some(session) = self.session_handle(session_id).await {
            let mut session = session.lock().await;
            session.info.status = BridgeSessionStatus::Failed;
            session.pending_approvals.clear();
            session.approval_tx = None;
            let _ = session.events_tx.send(BridgeEvent::SessionUpdated {
                session: session.info.clone(),
            });
            let _ = session.events_tx.send(BridgeEvent::Agent {
                session_id: session_id.to_string(),
                event: AgentEvent::Error { message },
            });
        }
    }

    async fn handle_agent_event(
        &self,
        session_id: &str,
        agent: &Agent<C>,
        parent_session_id: Option<&str>,
        parent_request_id: Option<&str>,
        event: &AgentEvent,
    ) {
        match event {
            AgentEvent::ApprovalRequired { request } => {
                if let Some(session) = self.session_handle(session_id).await {
                    let mut session = session.lock().await;
                    session.info.status = BridgeSessionStatus::AwaitingApproval;
                    session
                        .pending_approvals
                        .insert(request.id.clone(), request.clone());
                    let _ = session.events_tx.send(BridgeEvent::SessionUpdated {
                        session: session.info.clone(),
                    });
                }
            }
            AgentEvent::Done { result } => {
                self.finish_session(session_id, BridgeSessionStatus::Completed)
                    .await;
                if let (Some(parent_session_id), Some(parent_request_id)) =
                    (parent_session_id, parent_request_id)
                {
                    self.complete_parent_subagent(
                        parent_session_id,
                        parent_request_id,
                        agent,
                        result.clone(),
                    )
                    .await;
                }
            }
            AgentEvent::Error { message: _ } => {
                self.finish_session(session_id, BridgeSessionStatus::Failed)
                    .await;
                if let (Some(parent_session_id), Some(parent_request_id)) =
                    (parent_session_id, parent_request_id)
                {
                    self.complete_parent_subagent_error(
                        parent_session_id,
                        parent_request_id,
                        agent,
                    )
                    .await;
                }
            }
            AgentEvent::SubAgentRequested { .. } => {}
            _ => {}
        }
    }

    async fn finish_session(&self, session_id: &str, status: BridgeSessionStatus) {
        if let Some(session) = self.session_handle(session_id).await {
            let mut session = session.lock().await;
            session.info.status = status;
            session.pending_approvals.clear();
            session.approval_tx = None;
            session.task = None;
            let _ = session.events_tx.send(BridgeEvent::SessionUpdated {
                session: session.info.clone(),
            });
        }
    }

    async fn complete_parent_subagent(
        &self,
        parent_session_id: &str,
        parent_request_id: &str,
        child_agent: &Agent<C>,
        result: RunResult,
    ) {
        if let Some(parent) = self.session_handle(parent_session_id).await {
            let parent_agent = parent.lock().await.agent.clone();
            let text = if result.text.trim().is_empty() {
                "(no summary)".to_string()
            } else {
                result.text
            };
            let _ = parent_agent
                .complete_subagent(
                    parent_request_id,
                    crate::agent::loop_agent::SubAgentResult {
                        output: text,
                        is_error: false,
                    },
                )
                .await;
            let _ = child_agent;
        }
    }

    async fn complete_parent_subagent_error(
        &self,
        parent_session_id: &str,
        parent_request_id: &str,
        _child_agent: &Agent<C>,
    ) {
        if let Some(parent) = self.session_handle(parent_session_id).await {
            let parent_agent = parent.lock().await.agent.clone();
            let _ = parent_agent
                .complete_subagent(
                    parent_request_id,
                    crate::agent::loop_agent::SubAgentResult {
                        output: "Error: sub-agent session failed".to_string(),
                        is_error: true,
                    },
                )
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{LLMClient, StreamEvent};
    use async_trait::async_trait;
    use futures::Stream;
    use std::pin::Pin;

    #[derive(Clone, Default)]
    struct BridgeMockClient {
        events: Vec<StreamEvent>,
    }

    #[async_trait]
    impl LLMClient for BridgeMockClient {
        async fn create_message(
            &self,
            _system: &str,
            _messages: &[crate::agent::Message],
            _tools: &[serde_json::Value],
            _max_tokens: u32,
        ) -> Result<(String, Vec<crate::agent::ContentBlock>)> {
            Ok((
                "end_turn".to_string(),
                vec![crate::agent::ContentBlock::Text {
                    text: "ok".to_string(),
                }],
            ))
        }

        async fn create_message_stream(
            &self,
            _system: &str,
            _messages: &[crate::agent::Message],
            _tools: &[serde_json::Value],
            _max_tokens: u32,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
            let stream = futures::stream::iter(self.events.clone().into_iter().map(Ok));
            Ok(Box::pin(stream))
        }
    }

    #[tokio::test]
    async fn bridge_creates_and_runs_session() {
        let client = BridgeMockClient {
            events: vec![
                StreamEvent::TextDelta("hello".to_string()),
                StreamEvent::StopReason("end_turn".to_string()),
            ],
        };
        let bridge = LocalSessionBridge::new(client, Arc::new(Config::default()));
        let session = bridge
            .create_session(Some("demo".to_string()), AgentProfile::Default)
            .await
            .expect("create session");
        let mut rx = bridge.subscribe(&session.id).await.expect("subscribe");
        bridge
            .submit_input(&session.id, "hi".to_string())
            .await
            .expect("submit input");

        let mut saw_done = false;
        for _ in 0..8 {
            if let Ok(BridgeEvent::Agent {
                event: AgentEvent::Done { .. },
                ..
            }) = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .expect("timed receive")
            {
                saw_done = true;
                break;
            }
        }

        assert!(saw_done);
    }
}
