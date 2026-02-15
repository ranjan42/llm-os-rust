# Agent Runtime

The agent runtime is the heart of the LLM OS concept. In a traditional OS,
the userland is a collection of processes managed by a scheduler. In the
LLM OS, the userland is a single AI agent that owns the entire machine.

This document explains how the agent layer works and how it maps to
traditional OS concepts.

## The Mapping

The central idea is that every OS primitive has an LLM equivalent:

| OS Concept | LLM Equivalent | Implementation |
|---|---|---|
| Process | The Agent | `agent/mod.rs` — the single "super process" |
| RAM | Context Window | `agent/context.rs` — bounded token buffer |
| Disk / swap | Vector Store | Not yet implemented (would use RAG) |
| Syscall table | Tool Registry | `agent/tools.rs` — named capability map |
| Scheduler | Orchestrator | Not yet implemented |
| stdin | Keyboard interrupts | `interrupts.rs` — keyboard handler |
| stdout | VGA display | `vga_buffer.rs` — text-mode output |

## Context Window in Detail

The context window (`agent/context.rs`) is a bounded sequence of messages.
Think of it as the agent's working memory, directly analogous to RAM.

### Messages

Each message has three fields:
- **Role**: `System`, `User`, `Assistant`, or `Tool`
- **Content**: the text of the message
- **Token count**: estimated from content length (4 chars ≈ 1 token)

### Memory Hierarchy

The context window implements a simple memory hierarchy:

```
+---------------------------+
|      System Prompt        |  ← "L1 cache" — never evicted
+---------------------------+
|     Recent Messages       |  ← "RAM" — the context window
|   (User, Assistant, Tool) |
+---------------------------+
|     Evicted Messages      |  ← "Disk" — would go to vector store
+---------------------------+
```

When the context fills up (total tokens exceed `max_tokens`), the oldest
non-system message is removed. The system prompt is protected from
eviction because it defines the agent's identity and capabilities.

### Why This Matters

LLMs have a fixed context length. A 4096-token window can hold roughly
3000 words of conversation history. Once it fills up, you have to choose
what to keep and what to discard. This is exactly the same problem an OS
faces when physical RAM is full: which pages do you evict to disk?

The eviction policy here is simple FIFO (oldest first). A production
system would use something smarter — perhaps keeping messages that are
semantically relevant (using embedding similarity) or summarizing old
conversations before evicting them.

## Tool Registry in Detail

The tool registry (`agent/tools.rs`) maps tool names to implementations.
This is directly analogous to the kernel's syscall table, where syscall
numbers map to kernel functions.

### Current Tools

**Calculator (`calc`)**

The simplest tool — parses `"a op b"` and returns the result. Supports
`+`, `-`, `*`, `/` on 64-bit integers.

```
/tool calc 100 * 42
→ 100 * 42 = 4200
```

**Memory Store (`store`)**

Saves a key-value pair in a `BTreeMap`. This is the most primitive form
of persistent memory — the agent can remember facts across conversation
turns.

```
/tool store capital_france Paris
→ Stored: capital_france = Paris
```

**Memory Recall (`recall`)**

Looks up a key from the memory store.

```
/tool recall capital_france
→ capital_france = Paris
```

**Echo (`echo`)**

Echoes input back. Useful for testing that the tool dispatch system works.

### How Tool Calls Work

The agent's `process_input()` method checks if the input starts with
`/tool`. If so, it splits off the tool name and arguments, looks up the
tool in the registry, and calls `execute()`. The result (or error) is
added to the context window as a `Tool` message.

```
Input: "/tool calc 10 + 20"
  → tool_name = "calc"
  → tool_args = "10 + 20"
  → execute("calc", "10 + 20")
  → "10 + 20 = 30"
  → push Message { role: Tool, content: "10 + 20 = 30" }
```

### Future: LLM-Driven Tool Calls

Right now, tool calls are triggered by explicit `/tool` commands. In the
full implementation, the LLM itself would decide when to call tools:

1. User asks "What's 42 times 17?"
2. LLM generates: `{"tool": "calc", "args": "42 * 17"}`
3. Kernel parses the tool call, executes it, adds the result to context
4. LLM generates the final answer using the tool result

This is exactly how ChatGPT's function calling and tool use works. The
difference is that here, the tools are kernel-level capabilities — they
have direct hardware access and don't go through any OS sandboxing.

## What's Missing (Roadmap)

### Network Driver

The most critical missing piece. Without a network driver, there's no way
to reach an LLM inference API. The plan is to implement a virtio-net driver
(for QEMU) or an e1000 driver (Intel's classic NIC), add a minimal TCP/IP
stack, and make HTTP requests to an inference endpoint.

### Vector Store (Long-Term Memory)

When context window messages are evicted, they should be embedded and
stored in a vector database for later retrieval via semantic search. This
is the RAG (Retrieval-Augmented Generation) pattern — the agent's "disk"
for long-term memory.

### Orchestrator (Multi-Task Scheduling)

A single agent running a single inference loop is limiting. The
orchestrator would multiplex multiple "thinking threads" — one handling
user input, another doing background research, another monitoring system
health. Timer interrupts would drive the scheduling loop, similar to
how a traditional preemptive scheduler uses timer ticks.

### On-Device Inference

The ultimate goal: run the LLM directly on the hardware without any
network dependency. This would require porting a lightweight inference
engine (like llama.cpp or ONNX Runtime) to `no_std` Rust, which is a
substantial engineering challenge but would make the LLM OS truly
self-contained.
