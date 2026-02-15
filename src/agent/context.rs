//! Context Window — the agent's working memory ("RAM").
//!
//! The context window is a bounded sequence of messages (tokens). When full,
//! the oldest non-system messages are evicted — analogous to paging data
//! to disk when RAM is full.
//!
//! Memory Hierarchy:
//!   System Prompt  →  "L1 cache" (never evicted)
//!   Recent Messages →  "RAM" (the context window)
//!   Evicted History →  "Disk" (would go to vector store in full impl)

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::format;

/// Role of a message in the context window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// A single message in the context window.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub token_count: usize,
}

/// The context window — bounded working memory for the agent.
pub struct ContextWindow {
    messages: VecDeque<Message>,
    max_tokens: usize,
    current_tokens: usize,
    total_evicted: usize,
}

impl ContextWindow {
    /// Create a new context window with the given token limit.
    pub fn new(max_tokens: usize) -> Self {
        ContextWindow {
            messages: VecDeque::new(),
            max_tokens,
            current_tokens: 0,
            total_evicted: 0,
        }
    }

    /// Push a new message into the context window.
    ///
    /// If the window is full, evicts the oldest non-system messages
    /// until there is room. This is analogous to the OS paging data
    /// to disk when RAM is full.
    pub fn push_message(&mut self, role: Role, content: &str) {
        let token_count = estimate_tokens(content);

        // Eviction loop: make room if needed
        while self.current_tokens + token_count > self.max_tokens {
            if let Some(evicted) = self.evict_oldest() {
                self.total_evicted += 1;
                // In full implementation: summarize and store in vector DB
                crate::serial_println!(
                    "[context] Evicted {} message ({} tokens). Total evicted: {}",
                    evicted.role.as_str(),
                    evicted.token_count,
                    self.total_evicted
                );
            } else {
                break; // Only system prompt remains, can't evict
            }
        }

        self.current_tokens += token_count;
        self.messages.push_back(Message {
            role,
            content: String::from(content),
            token_count,
        });
    }

    /// Evict the oldest non-system message.
    fn evict_oldest(&mut self) -> Option<Message> {
        // Find the first non-system message
        let idx = self
            .messages
            .iter()
            .position(|m| m.role != Role::System)?;

        let msg = self.messages.remove(idx)?;
        self.current_tokens = self.current_tokens.saturating_sub(msg.token_count);
        Some(msg)
    }

    /// Get the current token count.
    pub fn current_tokens(&self) -> usize {
        self.current_tokens
    }

    /// Get the max token limit.
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Get the number of messages in the window.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get a human-readable status string.
    pub fn status(&self) -> String {
        format!(
            "Context Window Status:\n\
             Tokens: {}/{} ({:.1}% full)\n\
             Messages: {}\n\
             Evicted: {}",
            self.current_tokens,
            self.max_tokens,
            (self.current_tokens as f64 / self.max_tokens as f64) * 100.0,
            self.messages.len(),
            self.total_evicted,
        )
    }
}

/// Rough token estimation: ~4 characters per token (OpenAI's rule of thumb).
fn estimate_tokens(text: &str) -> usize {
    let count = text.len() / 4;
    if count == 0 { 1 } else { count }
}
