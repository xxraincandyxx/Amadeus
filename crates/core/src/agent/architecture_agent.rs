// @amadeus-header
// summary: Executes compiled agent architecture nodes through the production agent loop.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::architecture_agent
// - fn: crate::agent::architecture_agent::run_architecture_stream
// uses:
// - module: crate::agent::loop_agent
// - module: amadeus_runtime::architecture
// - protocol: legacy AgentEvent stream
// invariants:
// - One architecture run emits at most one terminal Done event.
// - Intermediate phase text is retained as workflow state rather than emitted as final text.
// side_effects:
// - Invokes configured models and tools.
// - May request delegated child-agent sessions.
// tests:
// - cmd: cargo test -p core architecture_agent --features full
// @end-amadeus-header

//! Adapter from compiled workflow nodes to the existing model and tool runtime.

use std::pin::Pin;
use std::sync::Arc;

use amadeus_runtime::{
    architecture_setting, compile_architecture, AgentArchitectureManifest, ArchitectureEdge,
    ArchitectureNode, ArchitectureNodeExecutor, ArchitectureNodeKind, ArchitectureNodeOutput,
    ArchitectureResources, ArchitectureRunState, WorkflowAgent, WorkflowAgentIdentity,
    WorkflowAgentRunStatus, WorkflowRunner,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde_json::Value;
use tokio::sync::{mpsc, Mutex};

use crate::agent::{Agent, AgentEvent, Message, RunResult, ToolCall};
use crate::client::LLMClient;
use crate::error::{AgentError, Result};

type ArchitectureEventStream = Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>;

struct CoreNodeExecutor<C: LLMClient> {
    agent: Agent<C>,
    events: mpsc::UnboundedSender<Result<AgentEvent>>,
    tool_calls: Arc<Mutex<Vec<ToolCall>>>,
}

#[async_trait]
impl<C> ArchitectureNodeExecutor for CoreNodeExecutor<C>
where
    C: LLMClient + Clone + 'static,
{
    async fn execute(
        &self,
        node: &ArchitectureNode,
        state: &ArchitectureRunState,
        outgoing: &[ArchitectureEdge],
    ) -> amadeus_runtime::NodeResult<ArchitectureNodeOutput> {
        let prompt = node_prompt(node, state, outgoing);
        self.agent
            .history()
            .write()
            .await
            .push(Message::user(&prompt));

        let mut stream = self.agent.run_stream();
        let mut result = None;
        while let Some(next) = stream.next().await {
            match next {
                Ok(AgentEvent::Done { result: completed }) => result = Some(completed),
                Ok(AgentEvent::TextDelta { .. } | AgentEvent::SessionSaved { .. }) => {}
                Ok(AgentEvent::Error { message }) => {
                    return Err(Box::new(AgentError::Api(message)));
                }
                Ok(event) => {
                    let _ = self.events.send(Ok(event));
                }
                Err(error) => return Err(Box::new(error)),
            }
        }

        let result = result.ok_or_else(|| Box::new(AgentError::StreamEndedUnexpectedly))?;
        self.tool_calls
            .lock()
            .await
            .extend(result.tool_calls.iter().cloned());
        if node.data.kind == ArchitectureNodeKind::Route {
            return Ok(ArchitectureNodeOutput {
                output: String::new(),
                transition: selected_transition(&result.text, outgoing),
            });
        }

        Ok(ArchitectureNodeOutput {
            output: result.text,
            transition: None,
        })
    }
}

/// Execute one architecture run and expose it through the legacy agent event protocol.
pub fn run_architecture_stream<C>(
    agent: Agent<C>,
    manifest: AgentArchitectureManifest,
    prompt: String,
) -> ArchitectureEventStream
where
    C: LLMClient + Clone + 'static,
{
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let task_events = events_tx.clone();
    let _task = tokio::spawn(async move {
        let workflow = match compile_architecture::<CoreNodeExecutor<C>>(&manifest) {
            Ok(workflow) => workflow,
            Err(error) => {
                let _ = task_events.send(Err(AgentError::InvalidResponse(error.to_string())));
                return;
            }
        };
        agent.history().write().await.push(Message::user(&prompt));

        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let resources = ArchitectureResources::new(CoreNodeExecutor {
            agent,
            events: task_events.clone(),
            tool_calls: Arc::clone(&tool_calls),
        });
        let workflow_agent = WorkflowAgent::new(
            WorkflowAgentIdentity::new(manifest.name.clone()),
            workflow,
            resources,
        )
        .with_runner(WorkflowRunner::new(manifest.max_transitions));

        match workflow_agent.run(ArchitectureRunState::new(prompt)).await {
            Ok(run) => match run.into_status() {
                WorkflowAgentRunStatus::Completed { state, .. } => {
                    let result = RunResult {
                        text: state.current_output,
                        tool_calls: tool_calls.lock().await.clone(),
                    };
                    if !result.text.is_empty() {
                        let _ = task_events.send(Ok(AgentEvent::TextDelta {
                            delta: result.text.clone(),
                        }));
                    }
                    let _ = task_events.send(Ok(AgentEvent::Done { result }));
                }
                WorkflowAgentRunStatus::Suspended(checkpoint) => {
                    let _ = task_events.send(Err(AgentError::InvalidResponse(format!(
                        "Architecture suspended at '{}': {}",
                        checkpoint.node_id(),
                        checkpoint.reason()
                    ))));
                }
            },
            Err(error) => {
                let _ = task_events.send(Err(AgentError::Api(error.to_string())));
            }
        }
    });
    drop(events_tx);

    Box::pin(futures::stream::unfold(
        events_rx,
        |mut receiver| async move { receiver.recv().await.map(|event| (event, receiver)) },
    ))
}

fn node_prompt(
    node: &ArchitectureNode,
    state: &ArchitectureRunState,
    outgoing: &[ArchitectureEdge],
) -> String {
    let configured = architecture_setting(node, "instruction")
        .or_else(|| architecture_setting(node, "criteria"))
        .or_else(|| architecture_setting(node, "capability"))
        .unwrap_or(node.data.description.as_str());
    let instruction = if configured.trim().is_empty() {
        default_instruction(node.data.kind)
    } else {
        configured
    };
    let prior = if state.current_output.trim().is_empty() {
        "No prior node output.".to_string()
    } else {
        format!("Current workflow result:\n{}", state.current_output)
    };

    if node.data.kind == ArchitectureNodeKind::Route {
        let labels = outgoing
            .iter()
            .map(|edge| edge.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return format!(
            "[Architecture node: {}]\n{}\nOriginal request:\n{}\n{}\nChoose the next transition. Reply with exactly one label and no other text: {}",
            node.data.label, instruction, state.input, prior, labels
        );
    }

    format!(
        "[Architecture node: {}]\n{}\nOriginal request:\n{}\n{}",
        node.data.label, instruction, state.input, prior
    )
}

fn default_instruction(kind: ArchitectureNodeKind) -> &'static str {
    match kind {
        ArchitectureNodeKind::Reason => "Reason about the request and produce the best candidate response.",
        ArchitectureNodeKind::Act => "Use the available tools to perform the required action, then report the result.",
        ArchitectureNodeKind::Plan => "Create an ordered, verifiable plan for the request.",
        ArchitectureNodeKind::Execute => "Execute the next incomplete plan step with the available tools.",
        ArchitectureNodeKind::Route => "Choose the transition that best matches the current workflow state.",
        ArchitectureNodeKind::Critique => "Critique the current result for correctness, completeness, and clarity.",
        ArchitectureNodeKind::Revise => "Revise the current result to address every actionable critique.",
        ArchitectureNodeKind::Delegate => "Delegate the requested work to a suitable specialist using the sub-agent tool, then summarize its result.",
        ArchitectureNodeKind::Review => "Review the delegated result against the request and acceptance criteria.",
        ArchitectureNodeKind::Synthesize => "Synthesize the accumulated work into the final answer.",
        ArchitectureNodeKind::Input
        | ArchitectureNodeKind::Observe
        | ArchitectureNodeKind::Approval
        | ArchitectureNodeKind::Output => "Continue the workflow.",
    }
}

fn selected_transition(text: &str, outgoing: &[ArchitectureEdge]) -> Option<String> {
    let trimmed = text.trim();
    if let Ok(Value::String(label)) = serde_json::from_str::<Value>(trimmed) {
        return matching_label(&label, outgoing);
    }
    if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(trimmed) {
        if let Some(label) = object.get("transition").and_then(Value::as_str) {
            return matching_label(label, outgoing);
        }
    }
    matching_label(trimmed, outgoing).or_else(|| {
        trimmed
            .lines()
            .rev()
            .find_map(|line| matching_label(line.trim(), outgoing))
    })
}

fn matching_label(label: &str, outgoing: &[ArchitectureEdge]) -> Option<String> {
    outgoing
        .iter()
        .find(|edge| edge.label.eq_ignore_ascii_case(label))
        .map(|edge| edge.label.clone())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};

    use amadeus_runtime::{ArchitectureNodeData, ArchitectureNodeKind};
    use futures::Stream;

    use super::*;
    use crate::agent::Config;
    use crate::client::StreamEvent;

    #[derive(Clone)]
    struct ScriptedClient {
        responses: Arc<Mutex<VecDeque<String>>>,
    }

    #[async_trait]
    impl LLMClient for ScriptedClient {
        async fn create_message(
            &self,
            _system: &str,
            _messages: &[Message],
            _tools: &[Value],
            _max_tokens: u32,
        ) -> Result<(String, Vec<crate::agent::ContentBlock>)> {
            Ok(("end_turn".to_string(), Vec::new()))
        }

        async fn create_message_stream(
            &self,
            _system: &str,
            _messages: &[Message],
            _tools: &[Value],
            _max_tokens: u32,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>> {
            let response = self.responses.lock().await.pop_front().unwrap_or_default();
            Ok(Box::pin(futures::stream::iter(vec![
                Ok(StreamEvent::TextDelta(response)),
                Ok(StreamEvent::StopReason("end_turn".to_string())),
            ])))
        }
    }

    fn node(id: &str, kind: ArchitectureNodeKind) -> ArchitectureNode {
        ArchitectureNode {
            id: id.to_string(),
            data: ArchitectureNodeData {
                kind,
                label: id.to_string(),
                description: String::new(),
                settings: BTreeMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn architecture_stream_emits_one_final_result() {
        let manifest = AgentArchitectureManifest {
            schema_version: 2,
            kind: "agent-architecture".to_string(),
            id: "reflection".to_string(),
            name: "Reflection".to_string(),
            preset: "reflection".to_string(),
            entry_node_id: "input".to_string(),
            max_transitions: 8,
            nodes: vec![
                node("input", ArchitectureNodeKind::Input),
                node("draft", ArchitectureNodeKind::Reason),
                node("route", ArchitectureNodeKind::Route),
                node("output", ArchitectureNodeKind::Output),
            ],
            edges: vec![
                ArchitectureEdge {
                    id: "e1".to_string(),
                    source: "input".to_string(),
                    target: "draft".to_string(),
                    label: "next".to_string(),
                },
                ArchitectureEdge {
                    id: "e2".to_string(),
                    source: "draft".to_string(),
                    target: "route".to_string(),
                    label: "next".to_string(),
                },
                ArchitectureEdge {
                    id: "e3".to_string(),
                    source: "route".to_string(),
                    target: "output".to_string(),
                    label: "accepted".to_string(),
                },
            ],
        };
        let client = ScriptedClient {
            responses: Arc::new(Mutex::new(VecDeque::from([
                "draft answer".to_string(),
                "accepted".to_string(),
            ]))),
        };
        let agent = Agent::builder(client, Arc::new(Config::default())).build();
        let mut stream = run_architecture_stream(agent, manifest, "question".to_string());
        let mut done = Vec::new();
        while let Some(event) = stream.next().await {
            if let AgentEvent::Done { result } = event.expect("architecture event") {
                done.push(result);
            }
        }

        assert_eq!(done.len(), 1);
        assert_eq!(done[0].text, "draft answer");
    }
}
