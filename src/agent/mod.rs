//! Agent Runtime — the AI-first userland.
//!
//! This module implements the core agent loop: gather input → build context →
//! invoke tools → emit output. In the LLM OS, the agent IS the userland.
//!
//! Submodules:
//! - `context` — Context window (the agent's working memory / "RAM")
//! - `tools`   — Tool registry (the agent's "syscall table")

pub mod context;
pub mod tools;

use alloc::string::String;
use alloc::vec::Vec;
use crate::{println, serial_println};

/// The Agent — the single "super process" that is the LLM OS userland.
pub struct Agent {
    pub context: context::ContextWindow,
    pub tool_registry: tools::ToolRegistry,
}

impl Agent {
    /// Create a new Agent with default configuration.
    pub fn new() -> Self {
        Agent {
            context: context::ContextWindow::new(4096), // 4K token context
            tool_registry: tools::ToolRegistry::new(),
        }
    }

    /// Boot the agent — load system prompt and register built-in tools.
    pub fn boot(&mut self) {
        serial_println!("[agent] Booting agent...");

        // Load system prompt into context window
        self.context.push_message(
            context::Role::System,
            "You are an AI agent running as the sole process on a bare-metal \
             x86_64 operating system. You have access to tools for calculation, \
             memory storage, and memory recall. Process user input and respond \
             helpfully.",
        );
        serial_println!("[agent] System prompt loaded ({} tokens used)", self.context.current_tokens());

        // Register built-in tools
        self.tool_registry.register(tools::BuiltinTool::Calculator);
        self.tool_registry.register(tools::BuiltinTool::MemoryStore);
        self.tool_registry.register(tools::BuiltinTool::MemoryRecall);
        self.tool_registry.register(tools::BuiltinTool::Echo);

        serial_println!(
            "[agent] {} tools registered: {:?}",
            self.tool_registry.tool_count(),
            self.tool_registry.tool_names()
        );

        println!("  Tools: {}", self.tool_registry.tool_count());
        println!("  Context: {}/{} tokens", self.context.current_tokens(), self.context.max_tokens());
    }

    /// Process a single input message and return the agent's response.
    ///
    /// In a full implementation, this would:
    /// 1. Add the input to the context window
    /// 2. Send the context to an LLM inference endpoint
    /// 3. Parse tool calls from the response
    /// 4. Execute tools and feed results back
    /// 5. Return the final response
    ///
    /// For now, we implement a simple command parser as a placeholder
    /// until network drivers enable remote inference.
    pub fn process_input(&mut self, input: &str) -> String {
        self.context.push_message(context::Role::User, input);
        serial_println!("[agent] Processing input: {}", input);

        // Simple command parsing (placeholder for LLM inference)
        let response = if input.starts_with("/tool ") {
            let tool_input = &input[6..];
            self.handle_tool_call(tool_input)
        } else if input.starts_with("/context") {
            self.context.status()
        } else if input.starts_with("/help") {
            String::from(
                "Available commands:\n\
                 /tool <name> <args>  — invoke a tool\n\
                 /context             — show context window status\n\
                 /help                — show this help\n\
                 (anything else)      — echoed back (LLM inference not yet connected)"
            )
        } else {
            // Echo back (placeholder until inference is connected)
            alloc::format!("[echo] {}", input)
        };

        self.context.push_message(context::Role::Assistant, &response);
        response
    }

    /// Parse and execute a tool call.
    fn handle_tool_call(&mut self, input: &str) -> String {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let tool_name = parts.get(0).unwrap_or(&"");
        let tool_args = parts.get(1).unwrap_or(&"");

        match self.tool_registry.execute(tool_name, tool_args) {
            Ok(result) => {
                self.context.push_message(context::Role::Tool, &result);
                result
            }
            Err(e) => alloc::format!("Tool error: {}", e),
        }
    }
}
