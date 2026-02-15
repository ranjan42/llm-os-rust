# LLM OS — An AI-First Operating System in Rust

> *"What happens when you invert the entire operating system?"*

A bare-metal x86_64 operating system kernel written in Rust, inspired by [Andrej Karpathy's "LLM OS" concept](https://twitter.com/karpathy). Instead of serving a human user through a shell, the kernel serves a single AI Agent that **is** the userland.

## Architecture

Every traditional OS concept maps to its LLM equivalent:

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

## Current Status

### ✅ Implemented
- Bare-metal x86_64 kernel booting via the `bootloader` crate
- GDT + TSS with interrupt stack table for safe double-fault handling
- IDT with exception handlers (breakpoint, double fault, page fault)
- PIC-based hardware interrupts (timer, keyboard)
- VGA text-mode output with colored text
- Serial port output for debug logging
- 4-level page table initialization
- Physical frame allocator from bootloader memory map
- 1 MiB kernel heap with linked-list allocator
- **Agent runtime** with:
  - Context window with token-based eviction policy
  - Tool registry with calculator, memory store/recall, echo
  - Command-based input processing (placeholder for LLM inference)

### 🚧 Roadmap
- [ ] Network driver (virtio-net or e1000) for LLM inference API
- [ ] Vector store for long-term memory (RAG)
- [ ] Orchestrator for multi-task scheduling
- [ ] Multimodal I/O (display rendering from agent output)
- [ ] On-device inference via ONNX Runtime or llama.cpp port

## Building

### Prerequisites

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

### Build & Run

```bash
# Build the kernel + bootable image
cargo bootimage

# Run in QEMU
qemu-system-x86_64 -drive format=raw,file=target/x86_64-llm-os/debug/bootimage-llm-os.bin

# Or use the cargo runner (configured in .cargo/config.toml)
cargo run
```

## Blog Post

Read the full design deep dive: [Building an AI-First Operating System in Rust from Scratch](https://ranjan42.github.io/)

## Inspired By

- [Andrej Karpathy — "LLM OS" concept](https://twitter.com/karpathy)
- [Writing an OS in Rust — Philipp Oppermann](https://os.phil-opp.com/)
- [go-dav-os — A freestanding OS kernel in Go](https://github.com/dmarro89/go-dav-os) (a project I contribute to)

## License

MIT
