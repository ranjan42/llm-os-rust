# LLM OS — An AI-First Operating System in Rust

A bare-metal x86_64 operating system kernel written in Rust, inspired by
[Andrej Karpathy's "LLM OS" concept](https://twitter.com/karpathy). The idea
is simple: instead of the kernel serving a human user through a shell, it serves
a single AI Agent that *is* the userland.

Every traditional OS primitive has an LLM equivalent — the CPU becomes the LLM,
RAM becomes the context window, disk becomes a vector store, and system calls
become tool invocations. This project builds that mapping from scratch on real
hardware.

## Architecture

```
+------------------------------------------------------------------+
|                        Agent Runtime                             |
|         (the single "super process" — the AI userland)           |
|                                                                  |
|   +------------------+  +----------------+  +----------------+   |
|   | Context Window   |  | Tool Registry  |  | Command Parser |   |
|   | (working memory, |  | (calculator,   |  | (input routing |   |
|   |  token buffer,   |  |  memory store, |  |  and dispatch) |   |
|   |  eviction policy)|  |  recall, echo) |  |                |   |
|   +------------------+  +----------------+  +----------------+   |
+----------------------------------+-------------------------------+
                                   |
                          tool calls / responses
                                   |
+----------------------------------v-------------------------------+
|                       Kernel Services                            |
|                                                                  |
|   +--------------+  +----------------+  +---------------------+  |
|   | Heap         |  | Memory Manager |  | Interrupt Handlers  |  |
|   | Allocator    |  | (4-level page  |  | (keyboard, timer,   |  |
|   | (1 MiB       |  |  tables, frame |  |  breakpoint, double |  |
|   |  linked-list)|  |  allocator)    |  |  fault, page fault) |  |
|   +--------------+  +----------------+  +---------------------+  |
|                                                                  |
|   +--------------+                                               |
|   | GDT + TSS    |                                               |
|   | (interrupt   |                                               |
|   |  stack table)|                                               |
|   +--------------+                                               |
+----------------------------------+-------------------------------+
                                   |
                            hardware access
                                   |
+----------------------------------v-------------------------------+
|                    Hardware Abstraction                           |
|                                                                  |
|   +----------+  +----------+  +----------+  +--------------+    |
|   | VGA Text |  | Serial   |  | PIC 8259 |  | Bootloader   |    |
|   | Display  |  | (UART    |  | (timer + |  | (memory map, |    |
|   |          |  |  16550)  |  |  keyboard)|  |  page tables)|    |
|   +----------+  +----------+  +----------+  +--------------+    |
+----------------------------------+-------------------------------+
                                   |
+----------------------------------v-------------------------------+
|                   Bare Metal x86_64 Hardware                     |
+------------------------------------------------------------------+
```

**How traditional OS concepts map to the LLM OS:**

| Traditional OS | LLM OS |
|---|---|
| CPU | LLM — processes tokens instead of machine instructions |
| RAM | Context Window — working memory for the current reasoning chain |
| Disk | Vector Store (RAG) — long-term memory via semantic retrieval |
| System Calls | Tool Invocations — calculator, web search, code execution |
| Scheduler | Orchestrator — multiplexes inference cycles across tasks |
| Peripherals | Multimodal I/O — keyboard, display, network |

## Project Structure

```
src/
├── main.rs          # Kernel entry point — boots hardware, starts agent
├── linker.ld        # Custom linker script — enforces 4K page alignment
├── lib.rs           # Kernel library — init, HLT loop, test infra
├── vga_buffer.rs    # VGA text-mode display driver
├── serial.rs        # UART 16550 serial port driver
├── gdt.rs           # Global Descriptor Table + TSS
├── interrupts.rs    # IDT, exception handlers, keyboard/timer IRQs
├── memory.rs        # Page table init + frame allocator
├── allocator.rs     # Kernel heap allocator (1 MiB linked-list)
└── agent/
    ├── mod.rs       # Agent runtime — the "super process"
    ├── context.rs   # Context window (the agent's "RAM")
    └── tools.rs     # Tool registry (the agent's "syscall table")
```

## What's Working

- Bare-metal x86_64 kernel booting via a **vendored & patched bootloader** (ensures ABI stability)
- **Custom linker script** enforcing 4K page alignment to prevent boot panics
- GDT + TSS with interrupt stack table for safe double-fault handling
- IDT with exception handlers (breakpoint, double fault, page fault)
- PIC-based hardware interrupts (timer, keyboard)
- VGA text-mode output with colored text
- Serial port output for debug logging
- 4-level page table initialization
- Physical frame allocator from bootloader memory map
- 1 MiB kernel heap with linked-list allocator
- Agent runtime with:
  - Context window with token-based eviction policy
  - Tool registry with calculator, memory store/recall, echo
  - Command-based input processing (placeholder for LLM inference)

## What's Next

- Network driver (virtio-net or e1000) for LLM inference API calls
- Vector store for long-term memory (RAG)
- Orchestrator for multi-task scheduling
- Multimodal I/O (display rendering from agent output)
- On-device inference via ONNX Runtime or llama.cpp port

## Building

You'll need a few things installed first:

```bash
# Rust nightly (auto-configured by rust-toolchain.toml)
rustup install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly

# Bootimage tool
cargo install bootimage

# QEMU for running the OS
brew install qemu  # macOS
# apt install qemu-system-x86 # Ubuntu
```

Then build and run:

```bash
# Build the kernel + bootable image
cargo bootimage

# Run in QEMU
qemu-system-x86_64 -drive format=raw,file=target/x86_64-llm-os/debug/bootimage-llm-os.bin

# Or use the cargo runner (configured in .cargo/config.toml)
cargo run
```

## Blog Post

Read the full design deep dive:
[Building an AI-First Operating System in Rust from Scratch](https://ranjan42.github.io/)

## Inspired By

- [Andrej Karpathy's "LLM OS" concept](https://twitter.com/karpathy)
- [Writing an OS in Rust by Philipp Oppermann](https://os.phil-opp.com/)
- [go-dav-os — A freestanding OS kernel in Go](https://github.com/dmarro89/go-dav-os) (a project I contribute to)

## License

MIT
