# Implementation Plan: Multi-Agent TUI with API-First Architecture

## Overview

This document details the implementation plan for adding multi-agent support to the Amadeus TUI. The architecture follows a strict API-first design where the TUI communicates with the agent core exclusively through HTTP endpoints.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              TUI Layer (src/ui/)                            │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  App                                                                │  │
│   │  - Uses ApiClient (HTTP) only                                       │  │
│   │  - Renders UI, handles input                                        │  │
│   │  - Commands: /new-agent, /agents, Tab to switch                     │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │ HTTP API (reqwest)
┌─────────────────────────────────┴───────────────────────────────────────────┐
│                            API Layer (src/api/)                            │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  New Endpoints:                                                      │  │
│   │  POST   /agents              - Create agent                         │  │
│   │  GET    /agents              - List all agents                     │  │
│   │  GET    /agents/:id           - Get agent info                      │  │
│   │  DELETE /agents/:id           - Kill agent                          │  │
│   │  POST   /agents/:id/chat      - Run prompt on specific agent        │  │
│   │  GET    /agents/:id/stream   - Stream events from agent             │  │
│   │  POST   /agents/:id/switch   - Switch active agent                  │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  AppState                                                            │  │
│   │  - supervisor: Arc<Supervisor<C>>  (existing)                       │  │
│   │  - agent_manager: AgentManager   (NEW)                               │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
┌─────────────────────────────────┴───────────────────────────────────────────┐
│                         Agent Core (src/agent/)                             │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  AgentManager (NEW)                                                 │  │
│   │  - Manages Vec<Agent>                                               │  │
│   │  - Handles call_peer routing between agents                         │  │
│   │  - Activates call_peer tool when 2+ agents exist                   │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  Agent (existing)                                                   │  │
│   │  - ReAct loop                                                       │  │
│   │  - Tool execution                                                   │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Agent Core (src/agent/)

### 1.1 Create `src/agent/profile.rs`

**Purpose:** Define agent profiles with different system prompts for different roles.

```rust
// ============================================================================
// FILE: src/agent/profile.rs
// ============================================================================

use serde::{Deserialize, Serialize};

/// Agent profile defines the role/specialization of an agent.
/// Each profile has a specific system prompt that shapes the agent's behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentProfile {
    /// Default agent - general purpose, current system prompt
    Default,
    /// Debugging specialist - focuses on error analysis, debugging
    Debug,
    /// Documentation specialist - focuses on docs, README, comments
    Docs,
    /// Code review specialist - focuses on PR reviews, code quality
    CodeReview,
    /// Custom profile with user-defined system prompt
    Custom(String),
}

impl AgentProfile {
    /// Get the system prompt for this profile.
    /// This prompt is prepended to the agent's conversation.
    pub fn system_prompt(&self) -> String {
        match self {
            AgentProfile::Default => Self::default_prompt(),
            AgentProfile::Debug => Self::debug_prompt(),
            AgentProfile::Docs => Self::docs_prompt(),
            AgentProfile::CodeReview => Self::code_review_prompt(),
            AgentProfile::Custom(custom) => custom.clone(),
        }
    }

    /// Display name for UI purposes.
    pub fn display_name(&self) -> &str {
        match self {
            AgentProfile::Default => "default",
            AgentProfile::Debug => "debug",
            AgentProfile::Docs => "docs",
            AgentProfile::CodeReview => "review",
            AgentProfile::Custom(_) => "custom",
        }
    }

    /// Default system prompt (current CLI agent prompt).
    fn default_prompt(&self) -> String {
        r#"You are Amadeus, an AI programming assistant.

# Core Identity
You are a powerful agent that helps users with software development tasks.

# Capabilities
- Read, write, and edit files
- Execute shell commands
- Search and analyze code
- Use tools to accomplish tasks

# Guidelines
- Think step by step before taking action
- Explain your reasoning before making changes
- Ask clarifying questions when needed
- Be precise and accurate in your responses"#.to_string()
    }

    /// Debugging specialist prompt.
    fn debug_prompt(&self) -> String {
        r#"You are Amadeus-Debug, an AI debugging specialist.

# Role
You specialize in debugging, error analysis, and problem diagnosis.

# Expertise
- Analyzing error messages and stack traces
- Identifying root causes of bugs
- Reading and understanding existing code
- Proposing targeted fixes
- Using debugging tools and techniques

# Approach
- First understand the error thoroughly
- Read relevant code to understand context
- Identify the root cause, not just symptoms
- Propose minimal, targeted fixes
- Explain the debugging process"#.to_string()
    }

    /// Documentation specialist prompt.
    fn docs_prompt(&self) -> String {
        r#"You are Amadeus-Docs, an AI documentation specialist.

# Role
You specialize in creating and improving documentation.

# Expertise
- Writing README files
- Creating API documentation
- Adding code comments
- Structuring documentation
- Markdown formatting

# Approach
- Keep documentation clear and concise
- Use appropriate formatting
- Focus on user-facing documentation
- Maintain consistency with existing docs"#.to_string()
    }

    /// Code review specialist prompt.
    fn code_review_prompt(&self) -> String {
        r#"You are Amadeus-Review, an AI code review specialist.

# Role
You specialize in code reviews and quality assessment.

# Expertise
- Identifying code smells
- Suggesting improvements
- Ensuring code quality
- Checking for edge cases
- Security considerations

# Approach
- Review code thoroughly but efficiently
- Focus on important issues first
- Suggest concrete improvements
- Be constructive and helpful
- Consider code maintainability"#.to_string()
    }
}

impl Default for AgentProfile {
    fn default() -> Self {
        AgentProfile::Default
    }
}

impl std::fmt::Display for AgentProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
```

---

### 1.2 Create `src/agent/manager.rs`

**Purpose:** Manage multiple agents, handle agent lifecycle, route calls between agents.

```rust
// ============================================================================
// FILE: src/agent/manager.rs
// ============================================================================

use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::profile::AgentProfile;
use crate::client::LLMClient;
use crate::core::id::AgentId;
use crate::error::{AgentError, Result};
use crate::tools::peer::{CallPeerTool, PeerInfo};

/// Status of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is idle, waiting for input
    Idle,
    /// Agent is currently processing a request
    Running,
    /// Agent has an error
    Error,
}

/// Information about an agent (returned to API/UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique agent identifier.
    pub id: AgentId,
    /// User-defined name for the agent.
    pub name: String,
    /// Agent profile/type.
    pub profile: AgentProfile,
    /// Current status.
    pub status: AgentStatus,
    /// Number of tasks completed.
    pub task_count: usize,
}

/// Manages multiple agents and coordinates between them.
pub struct AgentManager<C: LLMClient> {
    /// The LLM client shared by all agents.
    client: C,
    /// Configuration for agents.
    config: Arc<Config>,
    /// Active agents.
    agents: Vec<AgentHandle<C>>,
    /// Currently active agent index.
    active_index: usize,
    /// Counter for agent names.
    name_counter: usize,
}

struct AgentHandle<C: LLMClient> {
    /// The agent instance.
    agent: Agent<C>,
    /// User-defined name.
    name: String,
    /// Agent profile.
    profile: AgentProfile,
    /// Current status.
    status: AgentStatus,
    /// Number of completed tasks.
    task_count: usize,
}

impl<C: LLMClient + Clone + 'static> AgentManager<C> {
    /// Create a new agent manager.
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            agents: Vec::new(),
            active_index: 0,
            name_counter: 0,
        }
    }

    /// Create a new agent with the given profile.
    /// Returns the AgentId of the newly created agent.
    pub async fn create_agent(&mut self, name: Option<String>, profile: AgentProfile) -> Result<AgentId> {
        // Generate name if not provided
        let name = name.unwrap_or_else(|| {
            self.name_counter += 1;
            format!("{}-{}", profile.display_name(), self.name_counter)
        });

        // Determine if we need to enable call_peer
        let enable_call_peer = self.agents.len() >= 1; // Enable when 2+ agents

        // Create the agent with appropriate system prompt
        let agent = Agent::builder()
            .client(self.client.clone())
            .config(Arc::clone(&config))
            .system_prompt(profile.system_prompt())
            .build();

        let id = agent.id().clone();

        // Add to agents list
        self.agents.push(AgentHandle {
            agent,
            name,
            profile: profile.clone(),
            status: AgentStatus::Idle,
            task_count: 0,
        });

        // If this is the first agent, set as active
        if self.agents.len() == 1 {
            self.active_index = 0;
        }

        // Update call_peer for all agents if needed
        if enable_call_peer {
            self.update_peer_tools().await;
        }

        Ok(id)
    }

    /// List all active agents.
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .enumerate()
            .map(|(i, handle)| AgentInfo {
                id: handle.agent.id().clone(),
                name: handle.name.clone(),
                profile: handle.profile.clone(),
                status: if i == self.active_index {
                    AgentStatus::Running // Active agent is "running" from user perspective
                } else {
                    handle.status
                },
                task_count: handle.task_count,
            })
            .collect()
    }

    /// Get info for a specific agent.
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        self.agents
            .iter()
            .find(|h| h.agent.id() == agent_id)
            .map(|handle| AgentInfo {
                id: handle.agent.id().clone(),
                name: handle.name.clone(),
                profile: handle.profile.clone(),
                status: handle.status,
                task_count: handle.task_count,
            })
    }

    /// Get the currently active agent.
    pub fn active_agent(&self) -> Option<&Agent<C>> {
        self.agents.get(self.active_index).map(|h| &h.agent)
    }

    /// Get the currently active agent ID.
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.agents.get(self.active_index).map(|h| h.agent.id().clone())
    }

    /// Switch to a different agent by ID.
    pub fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        let index = self
            .agents
            .iter()
            .position(|h| h.agent.id() == agent_id)
            .ok_or_else(|| AgentError::AgentNotFound(agent_id.to_string()))?;

        self.active_index = index;
        Ok(())
    }

    /// Switch to the next agent.
    pub fn switch_next(&mut self) {
        if !self.agents.is_empty() {
            self.active_index = (self.active_index + 1) % self.agents.len();
        }
    }

    /// Switch to the previous agent.
    pub fn switch_prev(&mut self) {
        if !self.agents.is_empty() {
            self.active_index = if self.active_index == 0 {
                self.agents.len() - 1
            } else {
                self.active_index - 1
            };
        }
    }

    /// Kill (remove) an agent.
    pub fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        let index = self
            .agents
            .iter()
            .position(|h| h.agent.id() == agent_id)
            .ok_or_else(|| AgentError::AgentNotFound(agent_id.to_string()))?;

        // Don't allow killing the last agent
        if self.agents.len() == 1 {
            return Err(AgentError::AgentError(
                "Cannot kill the last agent".to_string(),
            ));
        }

        self.agents.remove(index);

        // Adjust active index if needed
        if self.active_index >= self.agents.len() {
            self.active_index = 0;
        }

        // Update peer tools - disable call_peer if only 1 agent left
        if self.agents.len() == 1 {
            // TODO: Rebuild tool registry without call_peer
        } else {
            self.update_peer_tools().await;
        }

        Ok(())
    }

    /// Get peer information for call_peer tool.
    /// Excludes the specified agent from the list.
    pub fn get_peers(&self, exclude_agent_id: &AgentId) -> Vec<PeerInfo> {
        self.agents
            .iter()
            .filter(|h| h.agent.id() != exclude_agent_id)
            .map(|h| PeerInfo {
                id: h.agent.id().clone(),
                name: h.name.clone(),
                profile: h.profile.display_name().to_string(),
                description: format!("{} agent", h.profile.display_name()),
            })
            .collect()
    }

    /// Check if call_peer should be enabled (2+ agents).
    pub fn is_call_peer_enabled(&self) -> bool {
        self.agents.len() >= 2
    }

    /// Update the call_peer tool for all agents.
    async fn update_peer_tools(&self) {
        for handle in &self.agents {
            let peers = self.get_peers(handle.agent.id());
            // TODO: Update agent's tool registry with new peer list
            let _ = peers;
        }
    }

    /// Get the total number of agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

// Add missing import
use serde::{Deserialize, Serialize};
```

---

### 1.3 Update `src/agent/mod.rs`

```rust
// ============================================================================
// FILE: src/agent/mod.rs (modify)
// ============================================================================

pub mod loop_agent;
pub mod manager;      // NEW
pub mod profile;      // NEW
pub mod supervisor;
pub mod worker;
pub mod config;
pub mod events;
pub mod messages;
pub mod compaction;

// Export new types
pub use manager::{AgentManager, AgentInfo, AgentStatus};
pub use profile::AgentProfile;
```

---

### 1.4 Update `src/tools/peer.rs`

**Purpose:** Make call_peer tool dynamic based on available agents.

```rust
// ============================================================================
// FILE: src/tools/peer.rs (modify)
// ============================================================================

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::id::AgentId;
use crate::error::{AgentError, Result};
use crate::tools::schema::call_peer_tool;
use crate::tools::tool_trait::Tool;

/// Information about a peer agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Agent ID.
    pub id: AgentId,
    /// Agent name.
    pub name: String,
    /// Agent profile.
    pub profile: String,
    /// Human-readable description.
    pub description: String,
}

/// Request to call another agent for help.
#[derive(Debug, Deserialize)]
pub struct CallPeerInput {
    /// The task to ask the peer agent to perform.
    pub task: String,
    /// Preferred capabilities (optional, for supervisor routing).
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Tool for requesting help from another agent.
/// Only enabled when 2+ agents exist.
#[derive(Clone)]
pub struct CallPeerTool {
    /// List of available peer agents.
    peers: Vec<PeerInfo>,
}

impl CallPeerTool {
    /// Create a new call peer tool.
    pub fn new() -> Self {
        Self { peers: Vec::new() }
    }

    /// Create with initial peer list.
    pub fn with_peers(peers: Vec<PeerInfo>) -> Self {
        Self { peers }
    }

    /// Update the peer list.
    pub fn set_peers(&mut self, peers: Vec<PeerInfo>) {
        self.peers = peers;
    }

    /// Get current peers.
    pub fn peers(&self) -> &[PeerInfo] {
        &self.peers
    }

    /// Check if there are any peers available.
    pub fn has_peers(&self) -> bool {
        !self.peers.is_empty()
    }

    /// Generate dynamic tool schema based on current peers.
    pub fn generate_schema(&self) -> Value {
        let mut schema = call_peer_tool();

        if self.peers.is_empty() {
            // No peers - disable the tool
            if let Some(obj) = schema.as_object_mut() {
                obj.insert(
                    "description".to_string(),
                    serde_json::json!("No other agents available. This tool is disabled."),
                );
            }
        } else {
            // Generate peer list description
            let peer_list: Vec<String> = self
                .peers
                .iter()
                .map(|p| format!("- [{}] {}: {}", p.name, p.profile, p.description))
                .collect();

            let description = format!(
                "Request help from another agent. Available agents:\n{}\n\nWhen calling, specify the agent name in the 'agent' field.",
                peer_list.join("\n")
            );

            if let Some(obj) = schema.as_object_mut() {
                obj.insert("description".to_string(), serde_json::json!(description));
            }
        }

        schema
    }
}

impl Default for CallPeerTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CallPeerTool {
    fn name(&self) -> &'static str {
        "call_peer"
    }

    fn schema(&self) -> &'static Value {
        // Return static schema - actual behavior controlled at registry level
        call_peer_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        // This will be handled by the agent manager
        // The actual routing happens in AgentManager::route_call_peer
        let parsed: CallPeerInput = serde_json::from_value(input)
            .map_err(|e| AgentError::ToolInput {
                tool: "call_peer".to_string(),
                reason: e.to_string(),
            })?;

        if self.peers.is_empty() {
            return Err(AgentError::ToolInput {
                tool: "call_peer".to_string(),
                reason: "No other agents available".to_string(),
            });
        }

        // The actual execution is delegated to the supervisor/manager
        // This is a placeholder - actual implementation in manager
        Ok(format!(
            "Delegating task to available agents: {}",
            parsed.task
        ))
    }
}
```

---

## Phase 2: API Layer (src/api/)

### 2.1 Update `src/api/types.rs`

Add the following types at the end of the file:

```rust
// ============================================================================
// FILE: src/api/types.rs (add at end)
// ============================================================================

/*
 * ============================================================================
 * MULTI-AGENT ENDPOINT TYPES
 * ============================================================================
 */

/// Request to create a new agent.
#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    /// Optional name for the agent.
    /// If not provided, a default name will be generated.
    #[serde(default)]
    pub name: Option<String>,
    /// Agent profile/type.
    /// Options: "default", "debug", "docs", "review", or "custom"
    #[serde(default = "default_profile")]
    pub profile: String,
}

fn default_profile() -> String {
    "default".to_string()
}

/// Response for agent creation.
#[derive(Debug, Serialize)]
pub struct CreateAgentResponse {
    /// The created agent's information.
    pub agent: AgentInfo,
}

/// Response listing all agents.
#[derive(Debug, Serialize)]
pub struct ListAgentsResponse {
    /// List of all active agents.
    pub agents: Vec<AgentInfo>,
    /// Currently active agent ID.
    pub active_agent_id: Option<String>,
}

/// Request to switch the active agent.
#[derive(Debug, Deserialize)]
pub struct SwitchAgentRequest {
    /// The agent ID to switch to.
    pub agent_id: String,
}

/// Response for agent switch.
#[derive(Debug, Serialize)]
pub struct SwitchAgentResponse {
    /// Whether the switch was successful.
    pub success: bool,
    /// The new active agent ID.
    pub active_agent_id: String,
}

/// Request to chat with a specific agent.
#[derive(Debug, Deserialize)]
pub struct AgentChatRequest {
    /// The message/prompt to send to the agent.
    pub message: String,
    /// Optional timeout for tool execution.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Response for agent chat (non-streaming).
#[derive(Debug, Serialize)]
pub struct AgentChatResponse {
    /// The agent's response content.
    pub content: String,
    /// Tool calls made during processing.
    pub tool_calls: Vec<ToolCall>,
    /// Why the agent stopped.
    pub stop_reason: String,
}

/// Request to kill an agent.
#[derive(Debug, Deserialize)]
pub struct KillAgentRequest {
    /// Reason for killing the agent (optional).
    #[serde(default)]
    pub reason: Option<String>,
}

/// Response for agent kill.
#[derive(Debug, Serialize)]
pub struct KillAgentResponse {
    /// Whether the kill was successful.
    pub success: bool,
}
```

---

### 2.2 Create `src/api/handlers/agents.rs`

**Purpose:** Implement agent management HTTP handlers.

```rust
// ============================================================================
// FILE: src/api/handlers/agents.rs (NEW)
// ============================================================================

use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::agent::manager::AgentManager;
use crate::agent::profile::AgentProfile;
use crate::api::http::AppState;
use crate::api::types::{
    AgentChatResponse, CreateAgentRequest, CreateAgentResponse, ErrorResponse,
    KillAgentRequest, KillAgentResponse, ListAgentsResponse, SwitchAgentRequest,
    SwitchAgentResponse, ToolCall,
};
use crate::client::LLMClient;

/// List all agents.
pub async fn list_agents<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<ListAgentsResponse>, Json<ErrorResponse>> {
    let agents = state.agent_manager.list_agents();
    let active_id = state.agent_manager.active_agent_id();

    Ok(Json(ListAgentsResponse {
        agents,
        active_agent_id: active_id.map(|id| id.to_string()),
    }))
}

/// Create a new agent.
pub async fn create_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, Json<ErrorResponse>> {
    let profile = match request.profile.as_str() {
        "default" => AgentProfile::Default,
        "debug" => AgentProfile::Debug,
        "docs" => AgentProfile::Docs,
        "review" | "code_review" => AgentProfile::CodeReview,
        _ => AgentProfile::Custom(format!("Custom profile: {}", request.profile)),
    };

    match state.agent_manager.create_agent(request.name, profile).await {
        Ok(agent_id) => {
            // Get the agent info
            if let Some(agent_info) = state.agent_manager.get_agent(&agent_id) {
                Ok(Json(CreateAgentResponse { agent: agent_info }))
            } else {
                Err(Json(ErrorResponse::new(
                    "AgentError",
                    "Failed to get agent info after creation",
                )))
            }
        }
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

/// Get info for a specific agent.
pub async fn get_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
) -> Result<Json<crate::agent::manager::AgentInfo>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    match state.agent_manager.get_agent(&agent_uuid) {
        Some(info) => Ok(Json(info)),
        None => Err(Json(ErrorResponse::new("AgentNotFound", "Agent not found"))),
    }
}

/// Delete (kill) an agent.
pub async fn kill_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(_request): Json<KillAgentRequest>,
) -> Result<Json<KillAgentResponse>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    match state.agent_manager.kill(&agent_uuid) {
        Ok(()) => Ok(Json(KillAgentResponse { success: true })),
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

/// Switch to a different agent.
pub async fn switch_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<SwitchAgentRequest>,
) -> Result<Json<SwitchAgentResponse>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = request
        .agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    match state.agent_manager.switch_to(&agent_uuid) {
        Ok(()) => {
            let new_active = state
                .agent_manager
                .active_agent_id()
                .map(|id| id.to_string())
                .unwrap_or_default();
            Ok(Json(SwitchAgentResponse {
                success: true,
                active_agent_id: new_active,
            }))
        }
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

/// Chat with a specific agent (non-streaming).
pub async fn agent_chat<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    // Get the agent
    let agent = {
        let agents = state.agent_manager.list_agents();
        agents
            .into_iter()
            .find(|a| a.id == agent_uuid)
            .ok_or_else(|| Json(ErrorResponse::new("AgentNotFound", "Agent not found")))?
    };

    // For now, we'll implement this by running the agent
    // In practice, this would call into the agent's run method
    // TODO: Implement actual agent execution

    Ok(Json(AgentChatResponse {
        content: format!("Agent '{}' received: {}", agent.name, request.message),
        tool_calls: vec![],
        stop_reason: "end_turn".to_string(),
    }))
}

/// Stream events from a specific agent.
pub async fn agent_stream<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, axum::response::sse::SseDisconnect>>>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    let message = params
        .get("message")
        .cloned()
        .unwrap_or_else(|| "Hello".to_string());

    // Verify agent exists
    let agents = state.agent_manager.list_agents();
    if !agents.iter().any(|a| a.id == agent_uuid) {
        return Err(Json(ErrorResponse::new("AgentNotFound", "Agent not found")));
    }

    // Create a stream that yields events
    // TODO: Connect to actual agent event stream
    let stream = stream::iter(vec![
        Ok(Event::default().data(format!("Agent {}: Processing '{}'", agent_id, message))),
        Ok(Event::default().data("Agent {}: Working on it...".to_string())),
        Ok(Event::default().data(format!("Agent {}: Done!".to_string()))),
    ])
    .map(Ok)
    .throttle(Duration::from_millis(100));

    Ok(Sse::new(stream))
}
```

---

### 2.3 Update `src/api/http.rs`

```rust
// ============================================================================
// FILE: src/api/http.rs (modify)
// ============================================================================

// Add new imports
use crate::agent::manager::AgentManager;

// Modify AppState
pub struct AppState<C: LLMClient> {
    pub supervisor: Arc<Supervisor<C>>,
    pub agent_manager: AgentManager<C>,  // NEW
}

// Modify create_router to add new routes
pub fn create_router<C: LLMClient + Clone + 'static>(state: Arc<AppState<C>>) -> Router {
    Router::new()
        // ... existing routes ...

        // NEW: Multi-agent endpoints
        .route("/agents", get(agents::list_agents))
        .route("/agents", post(agents::create_agent))
        .route("/agents/:id", get(agents::get_agent))
        .route("/agents/:id", delete(agents::kill_agent))
        .route("/agents/:id/switch", post(agents::switch_agent))
        .route("/agents/:id/chat", post(agents::agent_chat))
        .route("/agents/:id/stream", get(agents::agent_stream))

        // ... rest of router ...
}
```

---

### 2.4 Update `src/api/handlers/mod.rs`

```rust
// ============================================================================
// FILE: src/api/handlers/mod.rs (modify)
// ============================================================================

pub mod agents;    // NEW
pub mod approvals;
// ... existing modules
```

---

## Phase 3: TUI Layer (src/ui/)

### 3.1 Create `src/ui/api_client.rs`

**Purpose:** HTTP client wrapper for TUI to communicate with API.

```rust
// ============================================================================
// FILE: src/ui/api_client.rs (NEW)
// ============================================================================

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Information about an agent from API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub profile: String,
    pub status: String,
    pub task_count: usize,
}

/// Response from listing agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAgentsResponse {
    pub agents: Vec<AgentInfo>,
    pub active_agent_id: Option<String>,
}

/// Request to create an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: Option<String>,
    pub profile: String,
}

/// Response from creating an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentResponse {
    pub agent: AgentInfo,
}

/// API client for TUI to communicate with the server.
pub struct ApiClient {
    client: Client,
    base_url: String,
    /// Cached agent list (refreshed on demand).
    agents: Arc<RwLock<Vec<AgentInfo>>>,
    /// Currently active agent index.
    active_index: Arc<RwLock<usize>>,
}

impl ApiClient {
    /// Create a new API client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            agents: Arc::new(RwLock::new(Vec::new())),
            active_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Refresh the agent list from the server.
    pub async fn refresh_agents(&self) -> Result<ListAgentsResponse, String> {
        let response = self
            .client
            .get(format!("{}/agents", self.base_url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let data: ListAgentsResponse = response.json().await.map_err(|e| e.to_string())?;

        // Update cached agents
        *self.agents.write().await = data.agents.clone();

        // Update active index
        if let Some(ref active_id) = data.active_agent_id {
            if let Some(index) = data.agents.iter().position(|a| a.id == *active_id) {
                *self.active_index.write().await = index;
            }
        }

        Ok(data)
    }

    /// List all agents (cached).
    pub async fn list_agents(&self) -> Vec<AgentInfo> {
        self.agents.read().await.clone()
    }

    /// Get currently active agent.
    pub async fn active_agent(&self) -> Option<AgentInfo> {
        let index = *self.active_index.read().await;
        let agents = self.agents.read().await;
        agents.get(index).cloned()
    }

    /// Create a new agent.
    pub async fn create_agent(&self, name: Option<String>, profile: &str) -> Result<AgentInfo, String> {
        let request = CreateAgentRequest { name, profile: profile.to_string() };

        let response = self
            .client
            .post(format!("{}/agents", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to create agent: {}", response.status()));
        }

        let data: CreateAgentResponse = response.json().await.map_err(|e| e.to_string())?;

        // Refresh agent list
        self.refresh_agents().await?;

        Ok(data.agent)
    }

    /// Kill an agent.
    pub async fn kill_agent(&self, agent_id: &str) -> Result<(), String> {
        let response = self
            .client
            .delete(format!("{}/agents/{}", self.base_url, agent_id))
            .json(&serde_json::json!({ "reason": "User requested" }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to kill agent: {}", response.status()));
        }

        // Refresh agent list
        self.refresh_agents().await?;

        Ok(())
    }

    /// Switch to a different agent.
    pub async fn switch_agent(&self, agent_id: &str) -> Result<(), String> {
        let request = serde_json::json!({ "agent_id": agent_id });

        let response = self
            .client
            .post(format!("{}/agents/{}/switch", self.base_url, agent_id))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to switch agent: {}", response.status()));
        }

        // Update active index
        let agents = self.agents.read().await;
        if let Some(index) = agents.iter().position(|a| a.id == agent_id) {
            *self.active_index.write().await = index;
        }

        Ok(())
    }

    /// Switch to next agent.
    pub async fn switch_next(&self) -> Result<Option<AgentInfo>, String> {
        let mut index = *self.active_index.read().await;
        let agents = self.agents.read().await;

        if agents.is_empty() {
            return Ok(None);
        }

        index = (index + 1) % agents.len();
        let new_agent = agents.get(index).cloned();

        drop(agents);
        *self.active_index.write().await = index;

        if let Some(ref agent) = new_agent {
            self.switch_agent(&agent.id).await?;
        }

        Ok(new_agent)
    }

    /// Switch to previous agent.
    pub async fn switch_prev(&self) -> Result<Option<AgentInfo>, String> {
        let mut index = *self.active_index.read().await;
        let agents = self.agents.read().await;

        if agents.is_empty() {
            return Ok(None);
        }

        index = if index == 0 { agents.len() - 1 } else { index - 1 };
        let new_agent = agents.get(index).cloned();

        drop(agents);
        *self.active_index.write().await = index;

        if let Some(ref agent) = new_agent {
            self.switch_agent(&agent.id).await?;
        }

        Ok(new_agent)
    }

    /// Send a message to the active agent (non-streaming).
    pub async fn chat(&self, message: &str) -> Result<String, String> {
        let agent = self.active_agent().await.ok_or("No active agent")?;

        let request = serde_json::json!({
            "message": message
        });

        let response = self
            .client
            .post(format!("{}/agents/{}/chat", self.base_url, agent.id))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Chat failed: {}", response.status()));
        }

        #[derive(Deserialize)]
        struct ChatResponse {
            content: String,
        }

        let data: ChatResponse = response.json().await.map_err(|e| e.to_string())?;
        Ok(data.content)
    }

    /// Stream events from the active agent.
    pub async fn stream(&self, message: &str) -> Result<reqwest::EventSource, String> {
        let agent = self.active_agent().await.ok_or("No active agent")?;

        let url = format!(
            "{}/agents/{}/stream?message={}",
            self.base_url,
            agent.id,
            urlencoding::encode(message)
        );

        let event_source = self.client.get(&url).send().await.map_err(|e| e.to_string())?;

        Ok(event_source)
    }
}
```

---

### 3.2 Update `src/main.rs`

**Purpose:** Modify to support both server mode and TUI mode with API client.

```rust
// ============================================================================
// FILE: src/main.rs (modify)
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // ... existing setup ...

    // --- TUI MODE ---
    #[cfg(feature = "tui")]
    {
        // Option 1: Start embedded server and connect
        // Option 2: Connect to existing server
        // Option 3: Run without server (single agent mode)

        let use_api_mode = std::env::var("AMADEUS_API_URL").is_ok();

        if use_api_mode {
            // TUI connects to API server
            let api_url = std::env::var("AMADEUS_API_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

            let api_client = ApiClient::new(api_url);

            // Initialize with agent list
            if let Err(e) = api_client.refresh_agents().await {
                eprintln!("Warning: Could not connect to API server: {}", e);
                eprintln!("Starting in standalone mode...");
            }

            let mut app = App::with_api_client(api_client, workdir, model);
            app.run().await?;
        } else {
            // Original single-agent mode
            match provider {
                ClientKind::Anthropic(c) => {
                    let agent = Agent::new(c, sdk_config);
                    let mut app = App::new(agent, workdir, model);
                    app.run().await?;
                }
                ClientKind::OpenAI(c) => {
                    let agent = Agent::new(c, sdk_config);
                    let mut app = App::new(agent, workdir, model);
                    app.run().await?;
                }
            }
        }
    }

    Ok(())
}
```

---

### 3.3 Update `src/ui/app.rs`

**Purpose:** Modify to support multi-agent mode via ApiClient.

```rust
// ============================================================================
// FILE: src/ui/app.rs (modify key sections)
// ============================================================================

// Add to imports
use crate::ui::api_client::{ApiClient, AgentInfo};

// Modify App struct
pub struct App {
    // ... existing fields ...
    api_client: Option<ApiClient>,  // NEW: for multi-agent mode
    // Keep single agent for fallback
    agent: Option<Agent>,           // For standalone mode
}

// Add new constructor
impl App {
    /// Create app with API client (multi-agent mode).
    pub fn with_api_client(api_client: ApiClient, workdir: PathBuf, model: String) -> Self {
        Self {
            // ... existing fields ...
            api_client: Some(api_client),
            agent: None,
        }
    }

    /// Create app with single agent (standalone mode).
    pub fn new(agent: Agent, workdir: PathBuf, model: String) -> Self {
        Self {
            // ... existing fields ...
            api_client: None,
            agent: Some(agent),
        }
    }
}

// Modify key handling for Tab
fn handle_key(&mut self, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Tab => {
            // Switch to next agent
            if let Some(ref client) = self.api_client {
                match tokio::runtime::Handle::current().block_on(client.switch_next()) {
                    Ok(Some(agent)) => {
                        self.set_status(format!("Switched to agent: {}", agent.name));
                        return true;
                    }
                    Ok(None) => {
                        self.set_status("No agents available".to_string());
                    }
                    Err(e) => {
                        self.set_status(format!("Failed to switch: {}", e));
                    }
                }
            }
            true
        }
        KeyCode::BackTab => {
            // Switch to previous agent
            if let Some(ref client) = self.api_client {
                match tokio::runtime::Handle::current().block_on(client.switch_prev()) {
                    Ok(Some(agent)) => {
                        self.set_status(format!("Switched to agent: {}", agent.name));
                        return true;
                    }
                    _ => {}
                }
            }
            true
        }
        _ => false,
    }
}

// Modify input handling for commands
fn handle_input(&mut self, input: String) {
    if input.starts_with('/') {
        match input.as_str() {
            "/new-agent" => {
                self.show_agent_creation_dialog();
                return;
            }
            "/agents" => {
                self.show_agent_list();
                return;
            }
            _ => {}
        }
    }

    // Send to active agent (via API or direct)
    if let Some(ref client) = self.api_client {
        // Use API
        match tokio::runtime::Handle::current().block_on(client.chat(&input)) {
            Ok(response) => {
                self.add_message(AgentMessage {
                    role: "assistant".to_string(),
                    content: response,
                });
            }
            Err(e) => {
                self.set_status(format!("Error: {}", e));
            }
        }
    } else if let Some(ref agent) = self.agent {
        // Use direct agent
        // ... existing code ...
    }
}
```

---

### 3.4 Create `src/ui/components/agent_panel.rs`

**Purpose:** UI component to display agent list.

```rust
// ============================================================================
// FILE: src/ui/components/agent_panel.rs (NEW)
// ============================================================================

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::ui::api_client::AgentInfo;

/// Render the agent panel in the sidebar.
pub fn render_agent_panel<B: ratatui::backend::Backend>(
    frame: &mut Frame<B>,
    area: Rect,
    agents: &[AgentInfo],
    active_index: usize,
) {
    // Build list items
    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, agent)| {
            let prefix = if i == active_index { "●" } else { "○" };
            let status = if agent.status == "running" {
                "[█]"
            } else {
                "[░]"
            };

            let content = format!("{} {} {} {}", prefix, agent.name, status, agent.profile);

            ListItem::new(content).style(if i == active_index {
                Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Agents").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, area);
}

/// Render the agent creation dialog.
pub fn render_agent_dialog<B: ratatui::backend::Backend>(
    frame: &mut Frame<B>,
    area: Rect,
    selected_profile: usize,
) {
    let profiles = ["Default", "Debug", "Docs", "Code Review"];

    let content: Vec<Line> = vec![
        Line::from("Create New Agent"),
        Line::from(""),
        Line::from("Profile:"),
    ];

    // Render profile options
    for (i, profile) in profiles.iter().enumerate() {
        let prefix = if i == selected_profile { "▶" } else { " " };
        content.push(Line::from(format!("  {} {}", prefix, profile)));
    }

    content.push(Line::from(""));
    content.push(Line::from("[Enter] Create  [Esc] Cancel"));

    let paragraph = Paragraph::new(content)
        .block(Block::default().title("New Agent").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    // Center the dialog
    let dialog_width = 40.min(area.width);
    let dialog_height = 10.min(area.height);
    let x = (area.width - dialog_width) / 2;
    let y = (area.height - dialog_height) / 2;

    let dialog_area = Rect::new(area.x + x, area.y + y, dialog_width, dialog_height);
    frame.render_widget(paragraph, dialog_area);
}
```

---

## Summary of File Changes

### New Files

| File | Purpose |
|------|---------|
| `src/agent/profile.rs` | AgentProfile enum with system prompts |
| `src/agent/manager.rs` | AgentManager for multi-agent orchestration |
| `src/ui/api_client.rs` | HTTP client for TUI to talk to API |
| `src/ui/components/agent_panel.rs` | Agent panel UI component |

### Modified Files

| File | Changes |
|------|---------|
| `src/agent/mod.rs` | Export new types |
| `src/tools/peer.rs` | Dynamic call_peer tool |
| `src/api/types.rs` | Add agent request/response types |
| `src/api/handlers/mod.rs` | Add agents module |
| `src/api/handlers/agents.rs` | Agent HTTP handlers |
| `src/api/http.rs` | Add routes, update AppState |
| `src/main.rs` | Support API mode in TUI |
| `src/ui/app.rs` | Multi-agent UI support |

---

## Command Reference

### TUI Commands

| Command | Description |
|---------|-------------|
| `/new-agent` | Open dialog to create new agent |
| `/agents` | Show agent list |
| `Tab` | Switch to next agent |
| `Shift+Tab` | Switch to previous agent |
| `/kill <name>` | Kill an agent |

### Agent Profiles

| Profile | Description |
|---------|-------------|
| `default` | General purpose, current behavior |
| `debug` | Debugging specialist |
| `docs` | Documentation specialist |
| `review` | Code review specialist |

---

## Implementation Order

1. **Phase 1**: Agent Core (`src/agent/`)
   - Create `profile.rs`
   - Create `manager.rs`
   - Update `mod.rs`
   - Update `peer.rs`

2. **Phase 2**: API Layer (`src/api/`)
   - Update `types.rs`
   - Create `handlers/agents.rs`
   - Update `handlers/mod.rs`
   - Update `http.rs`

3. **Phase 3**: TUI (`src/ui/`)
   - Create `api_client.rs`
   - Update `main.rs`
   - Update `app.rs`
   - Create `components/agent_panel.rs`

---

## Testing Strategy

1. **Unit Tests**: Test AgentProfile, AgentManager logic
2. **Integration Tests**: Test API endpoints with mock agent manager
3. **TUI Tests**: Test key handling, command parsing
4. **E2E Tests**: Full flow with real API server
