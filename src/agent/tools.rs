//! Tool Registry — the agent's "syscall table."
//!
//! Each tool is a capability the agent can invoke. The registry maps
//! tool names to their implementations, analogous to how a syscall
//! table maps syscall numbers to kernel functions.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// Built-in tools available to the agent.
#[derive(Debug, Clone)]
pub enum BuiltinTool {
    /// Evaluate simple arithmetic expressions.
    Calculator,
    /// Store a key-value pair in memory.
    MemoryStore,
    /// Recall a value by key from memory.
    MemoryRecall,
    /// Echo input back (for testing).
    Echo,
}

impl BuiltinTool {
    pub fn name(&self) -> &str {
        match self {
            BuiltinTool::Calculator => "calc",
            BuiltinTool::MemoryStore => "store",
            BuiltinTool::MemoryRecall => "recall",
            BuiltinTool::Echo => "echo",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            BuiltinTool::Calculator => "Evaluate arithmetic: /tool calc <expr> (supports +, -, *, /)",
            BuiltinTool::MemoryStore => "Store key-value: /tool store <key> <value>",
            BuiltinTool::MemoryRecall => "Recall by key: /tool recall <key>",
            BuiltinTool::Echo => "Echo input: /tool echo <text>",
        }
    }
}

/// The tool registry — maps tool names to implementations.
pub struct ToolRegistry {
    tools: BTreeMap<String, BuiltinTool>,
    memory: BTreeMap<String, String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tools: BTreeMap::new(),
            memory: BTreeMap::new(),
        }
    }

    /// Register a built-in tool.
    pub fn register(&mut self, tool: BuiltinTool) {
        self.tools.insert(String::from(tool.name()), tool);
    }

    /// Execute a tool by name with the given arguments.
    pub fn execute(&mut self, name: &str, args: &str) -> Result<String, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Unknown tool '{}'. Available: {:?}", name, self.tool_names()))?
            .clone();

        match tool {
            BuiltinTool::Calculator => self.exec_calculator(args),
            BuiltinTool::MemoryStore => self.exec_memory_store(args),
            BuiltinTool::MemoryRecall => self.exec_memory_recall(args),
            BuiltinTool::Echo => Ok(format!("[echo] {}", args)),
        }
    }

    /// Get the number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Get the names of all registered tools.
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    // ─── Tool Implementations ──────────────────────────────────────

    /// Simple integer arithmetic calculator.
    fn exec_calculator(&self, expr: &str) -> Result<String, String> {
        let expr = expr.trim();

        // Parse "a <op> b" format
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(String::from("Usage: /tool calc <num> <op> <num> (e.g., /tool calc 42 + 7)"));
        }

        let a: i64 = parts[0]
            .parse()
            .map_err(|_| format!("Invalid number: {}", parts[0]))?;
        let op = parts[1];
        let b: i64 = parts[2]
            .parse()
            .map_err(|_| format!("Invalid number: {}", parts[2]))?;

        let result = match op {
            "+" => Ok(a + b),
            "-" => Ok(a - b),
            "*" => Ok(a * b),
            "/" => {
                if b == 0 {
                    Err(String::from("Division by zero"))
                } else {
                    Ok(a / b)
                }
            }
            _ => Err(format!("Unknown operator: {} (use +, -, *, /)", op)),
        }?;

        Ok(format!("{} {} {} = {}", a, op, b, result))
    }

    /// Store a key-value pair in the agent's memory.
    fn exec_memory_store(&mut self, args: &str) -> Result<String, String> {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err(String::from("Usage: /tool store <key> <value>"));
        }

        let key = String::from(parts[0]);
        let value = String::from(parts[1]);
        self.memory.insert(key.clone(), value.clone());
        Ok(format!("Stored: {} = {}", key, value))
    }

    /// Recall a value by key from the agent's memory.
    fn exec_memory_recall(&self, args: &str) -> Result<String, String> {
        let key = args.trim();
        match self.memory.get(key) {
            Some(value) => Ok(format!("{} = {}", key, value)),
            None => Err(format!("Key '{}' not found in memory", key)),
        }
    }
}
