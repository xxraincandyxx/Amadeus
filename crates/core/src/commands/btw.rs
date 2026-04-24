// @amadeus-header
// summary: Core-side side-question execution for the /btw command.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::commands::btw
// - fn: crate::commands::btw::answer_side_question
// - type: crate::commands::btw::SideQuestionOptions
// uses:
// - module: crate::agent::loop_agent
// - module: crate::agent::messages
// - module: crate::client
// - module: crate::error
// invariants:
// - Side questions reuse visible session context without mutating transcript history.
// - Side questions never expose tool schemas to the model.
// side_effects:
// - Sends one non-streaming LLM request through the configured client.
// tests:
// - cmd: cargo test -p core side_question --features full
// @end-amadeus-header

use crate::agent::loop_agent::Agent;
use crate::agent::messages::{ContentBlock, Message};
use crate::client::LLMClient;
use crate::error::Result;

#[derive(Debug, Clone, Default)]
pub struct SideQuestionOptions {
    pub in_flight_assistant_text: Option<String>,
}

/// Answer a `/btw` side question from existing session context without mutating transcript state.
pub async fn answer_side_question<C: LLMClient + Clone + 'static>(
    agent: &Agent<C>,
    question: &str,
    options: SideQuestionOptions,
) -> Result<String> {
    let question = question.trim();
    let history = agent.history();
    let config = agent.config();
    let client = agent.client();

    let mut messages = history.read().await.clone();
    if let Some(text) = options
        .in_flight_assistant_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        messages.push(Message::assistant(vec![ContentBlock::Text {
            text: text.to_string(),
        }]));
    }
    messages.push(Message::user(question));

    let system = format!(
        "{}\n\nYou are answering a /btw side question. Use only the conversation context already provided. Do not call tools. Do not ask follow-up questions. Reply with a single concise answer.",
        config.system_prompt(false)
    );
    let (_, content) = client.create_message(&system, &messages, &[], 1024).await?;
    Ok(collect_text(content))
}

fn collect_text(content: Vec<ContentBlock>) -> String {
    let text = content
        .into_iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "No /btw response returned.".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::Arc;

    use async_trait::async_trait;
    use futures::stream;
    use tokio::sync::Mutex;

    use super::{answer_side_question, SideQuestionOptions};
    use crate::agent::config::Config;
    use crate::agent::loop_agent::Agent;
    use crate::agent::messages::{ContentBlock, Message};
    use crate::client::{LLMClient, StreamEvent};
    use crate::error::Result;

    #[derive(Debug, Clone, Default)]
    struct RecordingState {
        calls: Vec<(String, Vec<Message>, Vec<serde_json::Value>)>,
    }

    #[derive(Debug, Clone)]
    struct RecordingClient {
        state: Arc<Mutex<RecordingState>>,
    }

    impl RecordingClient {
        fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(RecordingState::default())),
            }
        }

        async fn state(&self) -> RecordingState {
            self.state.lock().await.clone()
        }
    }

    #[async_trait]
    impl LLMClient for RecordingClient {
        async fn create_message(
            &self,
            system: &str,
            messages: &[Message],
            tools: &[serde_json::Value],
            _max_tokens: u32,
        ) -> Result<(String, Vec<ContentBlock>)> {
            self.state.lock().await.calls.push((
                system.to_string(),
                messages.to_vec(),
                tools.to_vec(),
            ));
            Ok((
                "end_turn".to_string(),
                vec![ContentBlock::Text {
                    text: "The file is docs/TMUX_TEST_FLOW.md.".to_string(),
                }],
            ))
        }

        async fn create_message_stream(
            &self,
            _system: &str,
            _messages: &[Message],
            _tools: &[serde_json::Value],
            _max_tokens: u32,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<StreamEvent>> + Send>>> {
            Ok(Box::pin(stream::iter(vec![Ok(StreamEvent::StopReason(
                "end_turn".to_string(),
            ))])))
        }
    }

    #[tokio::test]
    async fn side_question_uses_history_without_mutating_it() {
        let client = RecordingClient::new();
        let agent = Agent::builder(client.clone(), Arc::new(Config::default())).build();
        let history = agent.history();
        {
            let mut history = history.write().await;
            history.push(Message::user("we discussed tmux docs"));
        }
        let before = history.read().await.len();

        let answer = answer_side_question(
            &agent,
            "what file describes tmux testing?",
            SideQuestionOptions::default(),
        )
        .await
        .expect("side question should succeed");

        assert_eq!(answer, "The file is docs/TMUX_TEST_FLOW.md.");
        assert_eq!(history.read().await.len(), before);

        let state = client.state().await;
        assert_eq!(state.calls.len(), 1);
        let (system, messages, tools) = &state.calls[0];
        assert!(system.contains("/btw side question"));
        assert!(tools.is_empty());
        assert_eq!(messages.len(), before + 1);
    }

    #[tokio::test]
    async fn side_question_can_include_in_flight_assistant_text() {
        let client = RecordingClient::new();
        let agent = Agent::builder(client.clone(), Arc::new(Config::default())).build();

        answer_side_question(
            &agent,
            "repeat the current file name",
            SideQuestionOptions {
                in_flight_assistant_text: Some(
                    "I am currently reading docs/TMUX_TEST_FLOW.md".to_string(),
                ),
            },
        )
        .await
        .expect("side question should succeed");

        let state = client.state().await;
        let (_, messages, _) = &state.calls[0];
        assert!(matches!(
            messages.first().and_then(|message| message.content.first()),
            Some(ContentBlock::Text { text }) if text.contains("docs/TMUX_TEST_FLOW.md")
        ));
    }
}
