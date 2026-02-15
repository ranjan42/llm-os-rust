# LLM OS Documentation

This directory contains detailed documentation for every component of the
LLM OS kernel. If you are reading the code for the first time, start with
the boot sequence walkthrough below, then explore individual modules as
needed.

## Contents

- [Boot Sequence](boot-sequence.md) — what happens from power-on to agent startup
- [Kernel Modules](kernel-modules.md) — detailed breakdown of every source file
- [Build System](build-system.md) — how the toolchain, target spec, and bootimage fit together
- [Agent Runtime](agent-runtime.md) — how the AI agent layer works

## Quick Orientation

The kernel lives in `src/`. There are two kinds of files:

**Hardware layer** — the infrastructure that makes a bare-metal x86_64
machine usable:

| File | Purpose |
|---|---|
| `main.rs` | Kernel entry point, boot sequence |
| `lib.rs` | Shared kernel infrastructure, init, test harness |
| `gdt.rs` | CPU segmentation and task state segment |
| `interrupts.rs` | Exception and hardware interrupt handlers |
| `memory.rs` | Page tables and physical frame allocator |
| `allocator.rs` | Kernel heap (dynamic memory) |
| `vga_buffer.rs` | Text-mode display driver |
| `serial.rs` | UART serial port driver |

**Agent layer** — the AI-first userland that sits on top of the hardware:

| File | Purpose |
|---|---|
| `agent/mod.rs` | Agent struct, boot, input loop |
| `agent/context.rs` | Context window (bounded message buffer with eviction) |
| `agent/tools.rs` | Tool registry (calculator, memory, echo) |
