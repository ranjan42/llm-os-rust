# Kernel Modules

A file-by-file walkthrough of every source file in the project. Each section
explains what the file does, why it exists, and how it connects to the rest of
the kernel.

---

## `src/main.rs` — Kernel Entry Point

**Lines:** 90  
**Purpose:** The first Rust code that runs after the bootloader hands off
control. Orchestrates the entire boot sequence.

### Key Attributes

```rust
#![no_std]    // no standard library — we're on bare metal
#![no_main]   // no main() — the bootloader calls our entry_point! instead
```

The `entry_point!(kernel_main)` macro (from the `bootloader` crate) marks
`kernel_main` as the entry point and type-checks that the function signature
matches what the bootloader expects: `fn(&'static BootInfo) -> !`.

### Boot Sequence

`kernel_main` runs through three phases in order:

1. **Hardware init** — calls `llm_os::init()`, which sets up the GDT, IDT,
   and PIC.
2. **Memory setup** — initializes the page table mapper and frame allocator
   from the bootloader's memory map, then sets up the 1 MiB kernel heap.
3. **Agent startup** — creates the `Agent`, calls `boot()` to load the
   system prompt and register tools, then enters the halt loop.

### Panic Handlers

The file defines two panic handlers, selected at compile time:

- **Normal mode** (`#[cfg(not(test))]`): prints "KERNEL PANIC" to both
  the VGA screen and serial port, then enters the halt loop.
- **Test mode** (`#[cfg(test)]`): prints the panic info to serial and
  exits QEMU with a failure code.

---

## `src/lib.rs` — Kernel Library

**Lines:** 105  
**Purpose:** Shared infrastructure used by both `main.rs` and the test
harness. This is the crate root for the `llm_os` library.

### What It Provides

- **`init()`** — the single function that initializes all hardware. Calls
  `gdt::init()`, `interrupts::init_idt()`, initializes the PIC, and enables
  CPU interrupts. Every binary in this project calls this first.

- **`hlt_loop()`** — an infinite loop that uses the `HLT` instruction to
  sleep until the next interrupt. Used instead of a spin loop to save power.

- **Test infrastructure** — a custom test framework since we can't use the
  standard one in `no_std`. Includes:
  - `Testable` trait — wraps any `Fn()` to print the test name and [ok]/[failed]
  - `test_runner()` — runs all tests sequentially, prints results to serial,
    and exits QEMU with a success/failure code
  - `test_panic_handler()` — catch panics during tests and exit with failure
  - `QemuExitCode` — writes to port `0xf4` to signal QEMU to exit with a
    specific code (configured via the `isa-debug-exit` device)

- **Module declarations** — `pub mod agent`, `pub mod allocator`, etc. This
  is where all the kernel modules are wired together.

### Feature Flags

```rust
#![feature(custom_test_frameworks)]  // use our own test runner
#![feature(abi_x86_interrupt)]       // needed for interrupt handler functions
```

Both are nightly-only features, which is why we need a nightly toolchain.

---

## `src/gdt.rs` — Global Descriptor Table

**Lines:** 60  
**Purpose:** Set up CPU segmentation and the Task State Segment (TSS).

### Why It Exists

In 64-bit long mode, segmentation is mostly disabled — the CPU ignores
segment base and limit fields. But we still need a GDT for two reasons:

1. **The TSS** — the Task State Segment provides the Interrupt Stack Table
   (IST), which lets us define clean stacks for specific interrupts. Without
   this, a stack overflow triggers a double fault on a corrupt stack, which
   immediately triple-faults and reboots the machine.

2. **Ring transitions** — if we ever add user-mode agents (ring 3), the
   GDT defines the code/data segments for ring switching.

### Implementation Details

The GDT is built using `lazy_static!` because the TSS must be initialized
at runtime (its stack pointer depends on static memory layout).

**TSS setup:**
- Allocates a 20 KiB stack (`4096 * 5` bytes) as a static `[u8]` array
- Sets `interrupt_stack_table[0]` to point to the *top* of this stack
  (stacks grow downward on x86)
- This stack is used exclusively by the double-fault handler

**GDT entries:**
- `kernel_code_segment()` — the ring-0 code segment
- `tss_segment(&TSS)` — reference to the TSS

**`init()`:**
- Loads the GDT with `GDT.0.load()`
- Sets the CS register to the kernel code segment
- Loads the TSS selector with `load_tss()`

---

## `src/interrupts.rs` — Interrupt Handlers

**Lines:** 135  
**Purpose:** Define the Interrupt Descriptor Table and all interrupt/exception
handlers.

### PIC Configuration

The 8259 Programmable Interrupt Controller is configured with:
- PIC1 offset: 32 (IRQs 0-7 map to vectors 32-39)
- PIC2 offset: 40 (IRQs 8-15 map to vectors 40-47)

This avoids collision with CPU exception vectors 0-31.

The `PICS` static is a `spin::Mutex<ChainedPics>` — a spinlock-protected
pair of PIC controllers.

### Interrupt Index Enum

```rust
pub enum InterruptIndex {
    Timer = 32,    // PIC_1_OFFSET + 0
    Keyboard = 33, // PIC_1_OFFSET + 1
}
```

### Exception Handlers

**Breakpoint (`int3`)**  
Prints the stack frame to the VGA display. This is mostly useful for testing
that the IDT works. The CPU resumes execution after the handler returns.

**Double Fault**  
Fires when an exception occurs while handling another exception (or when
certain exception combinations happen). This handler panics — there is no
recovery from a double fault. It runs on its own IST stack (index 0) so
it can handle stack overflows safely.

**Page Fault**  
Fires when the CPU accesses a virtual address that isn't mapped, or when
a page access violates the page permissions. The handler prints the faulting
address (from the CR2 register), the error code, and the stack frame, then
enters the halt loop.

### Hardware Interrupt Handlers

**Timer (IRQ 0)**  
Fires roughly 18.2 times per second from the PIT (Programmable Interval
Timer). Currently a no-op — just sends the end-of-interrupt signal. In
the future, this will drive the orchestrator's scheduling loop.

**Keyboard (IRQ 1)**  
Reads a scancode from I/O port `0x60`, decodes it using the `pc-keyboard`
crate (US 104-key layout, scancode set 1), and prints the resulting
character to the VGA buffer. Each key press fires one interrupt.

Both hardware handlers must call `PICS.lock().notify_end_of_interrupt()`
at the end, or the PIC will stop sending further interrupts on that line.

### Thread Safety

The IDT and keyboard state are `lazy_static!` statics. The print functions
disable interrupts while holding the VGA/serial locks
(`interrupts::without_interrupts()`) to prevent deadlocks if an interrupt
fires while we're in the middle of printing.

---

## `src/memory.rs` — Memory Management

**Lines:** 73  
**Purpose:** Initialize the page table mapper and provide a physical frame
allocator.

### How Virtual Memory Works Here

The bootloader sets up 4-level paging and maps all physical memory at a
known virtual offset. For example, if the offset is `0x0000_1000_0000_0000`,
then physical address `0x1000` is accessible at virtual address
`0x0000_1000_0000_1000`.

`memory::init()` reads the CR3 register to find the physical address of
the level-4 page table, converts it to a virtual address using the offset,
and wraps it in `OffsetPageTable`. This gives us the standard `Mapper`
trait for creating and modifying page mappings.

### BootInfoFrameAllocator

This struct wraps the bootloader's memory map and implements the
`FrameAllocator` trait. When asked for a frame:

1. It filters the memory map for regions of type `Usable`
2. It generates 4 KiB-aligned addresses from each usable region
3. It returns the Nth frame (tracked by `self.next`) and increments

This is a simple linear allocator — it never frees frames. That's fine for
now since the only consumer is the heap setup, which allocates 256 frames
once during boot and never releases them.

### Safety

Both `init()` and `BootInfoFrameAllocator::init()` are `unsafe` because
the caller must guarantee that:
- The physical memory offset is correct (wrong offset = corrupt page tables)
- The memory map is accurate (allocating a "usable" frame that's actually
  in use would corrupt data)

---

## `src/allocator.rs` — Heap Allocator

**Lines:** 61  
**Purpose:** Give the kernel a heap so it can use `Vec`, `String`, `Box`,
and other dynamically-allocated types.

### Configuration

```rust
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB
```

The heap lives at a virtual address far from the kernel code and stack,
in a currently-unused part of the 48-bit virtual address space.

### How `init_heap()` Works

1. Calculates the page range from `HEAP_START` to `HEAP_START + HEAP_SIZE`
2. For each 4 KiB page in the range:
   - Allocates a physical frame from the `BootInfoFrameAllocator`
   - Maps the virtual page to the physical frame with `PRESENT | WRITABLE`
   - Flushes the TLB entry
3. Initializes the `linked_list_allocator` over the mapped region

### The Allocator Itself

We use `linked_list_allocator::LockedHeap` as the `#[global_allocator]`.
It's a classic linked-list free-list allocator wrapped in a spinlock.
When you call `alloc::vec![1, 2, 3]`, the compiler routes the allocation
through this allocator.

The linked-list allocator is simple but not fast — O(n) allocation time
where n is the number of free blocks. For a kernel this small, that's fine.
A production kernel would use a slab allocator or buddy allocator instead.

### What Uses the Heap

In the LLM OS, the heap backs:
- `VecDeque<Message>` — the agent's context window
- `BTreeMap<String, String>` — the tool registry and memory store
- `String` — tool call arguments, responses, formatted output
- `Vec<&str>` — input parsing (split, collect)

---

## `src/vga_buffer.rs` — VGA Text Display

**Lines:** 166  
**Purpose:** Write colored text to the screen using the VGA text-mode
buffer at physical address `0xB8000`.

### How VGA Text Mode Works

The VGA controller reserves a memory-mapped region at `0xB8000`. Each
character on screen is represented by two bytes:

| Byte | Contents |
|---|---|
| 0 | ASCII character code |
| 1 | Color attribute: `(background << 4) | foreground` |

The buffer is 80 columns by 25 rows = 4000 bytes. Writing a byte to
this memory region immediately changes what's displayed on screen.

### Color System

The `Color` enum maps the 16 standard VGA colors (0-15). The `ColorCode`
struct packs foreground and background into a single byte.

Default color: **LightCyan on Black** (foreground 11, background 0).

### Writer

The `Writer` struct tracks the current column position and writes
characters one byte at a time:

- Printable ASCII (0x20-0x7E) and newlines are written directly
- Non-ASCII bytes are replaced with `0xFE` (a filled square placeholder)
- When the cursor reaches column 80, it wraps to a new line
- `new_line()` shifts all rows up by one and clears the bottom row

The `Writer` implements `core::fmt::Write`, so it supports all the
standard formatting macros (`write!`, `writeln!`).

### Volatile Writes

Each cell in the buffer is `Volatile<ScreenChar>`. This prevents the
compiler from optimizing away writes to the VGA buffer — since the
compiler can't see anyone "reading" these memory locations, it might
otherwise decide the writes are unnecessary and eliminate them.

### Global Access

The writer is wrapped in `lazy_static! { pub static ref WRITER: Mutex<Writer> }`.
This gives us a global, thread-safe writer that any module can use
through the `print!` and `println!` macros.

The `_print()` function disables interrupts while holding the lock to
prevent deadlocks: if a timer interrupt fired while we were printing,
and the timer handler also tried to print, it would deadlock on the mutex.

---

## `src/serial.rs` — Serial Port Driver

**Lines:** 48  
**Purpose:** Send debug output to the host machine over the serial port.

### Why Serial?

When running in QEMU, serial output goes directly to the host terminal
(`-serial stdio`). This is useful for:
- Debug logging that doesn't clutter the VGA display
- Test output (the test framework reports results over serial)
- Communication with the host before any display setup

### Implementation

The serial port is a UART 16550 at I/O port `0x3F8` (COM1). We use the
`uart_16550` crate, which handles the register-level programming. The
port is initialized once and stored in a `lazy_static! Mutex<SerialPort>`.

The `serial_print!` and `serial_println!` macros mirror the VGA macros
but write to serial instead. Like the VGA printer, `_print()` disables
interrupts while holding the lock.

---

## `src/agent/mod.rs` — Agent Runtime

**Lines:** 115  
**Purpose:** The top-level agent struct — the "super process" that is the
LLM OS's entire userland.

### The Agent Struct

```rust
pub struct Agent {
    pub context: context::ContextWindow,
    pub tool_registry: tools::ToolRegistry,
}
```

The agent has two components: a context window (its working memory) and
a tool registry (its available actions).

### Boot Sequence

`agent.boot()`:
1. Pushes a system prompt into the context window. The system prompt
   tells the agent what it is and what tools it has.
2. Registers four built-in tools: `calc`, `store`, `recall`, `echo`.
3. Prints the tool count and token usage to the VGA display.

### Input Processing

`process_input(input)` is the main entry point for handling user input.
Currently it uses a simple command parser as a placeholder:

- `/tool <name> <args>` — invoke a registered tool
- `/context` — show context window status (tokens used, messages, evictions)
- `/help` — print available commands
- Anything else — echo back the input

In the future, this will send the full context window to an LLM
inference endpoint (over the network) and parse tool calls from the
response.

### How It Would Work With a Real LLM

The intended flow:

```
User types → keyboard interrupt → agent.process_input()
  → Build context (system prompt + message history)
  → Send to inference API (over network)
  → LLM responds (possibly with tool calls)
  → Execute tools, feed results back to LLM
  → Final response displayed on VGA
```

The agent loop is already structured for this — the missing piece is the
network driver.

---

## `src/agent/context.rs` — Context Window

**Lines:** 144  
**Purpose:** A bounded message buffer that acts as the agent's working
memory, analogous to RAM in a traditional OS.

### Design

The context window is a `VecDeque<Message>` with a maximum token count.
Each message has a role (`System`, `User`, `Assistant`, `Tool`), content
string, and an estimated token count.

**Token estimation:** `len / 4` (OpenAI's rule of thumb: ~4 chars per
token). Minimum 1 token per message.

### Eviction Policy

When a new message would push the total token count over the limit,
the context window evicts the oldest non-system message. This is
directly analogous to the OS paging data to disk when RAM is full:

- **System prompt** = "L1 cache" (never evicted)
- **Recent messages** = "RAM" (the context window)
- **Evicted messages** = "disk" (would go to a vector store in the full
  implementation)

Eviction is logged to serial so you can watch it happening.

### Public API

- `push_message(role, content)` — add a message, evicting old ones if needed
- `current_tokens()` — how many tokens are currently in the window
- `max_tokens()` — the capacity limit
- `message_count()` — number of messages in the buffer
- `status()` — human-readable string showing usage percentage and counts

---

## `src/agent/tools.rs` — Tool Registry

**Lines:** 149  
**Purpose:** A map from tool names to implementations, analogous to the
kernel's syscall table.

### Built-in Tools

| Name | Command | What It Does |
|---|---|---|
| `calc` | `/tool calc 42 + 7` | Integer arithmetic (+, -, *, /) |
| `store` | `/tool store pi 3.14` | Save a key-value pair in memory |
| `recall` | `/tool recall pi` | Look up a key from memory |
| `echo` | `/tool echo hello` | Echo input back (testing) |

### Implementation

The `ToolRegistry` holds two `BTreeMap`s:
- `tools: BTreeMap<String, BuiltinTool>` — maps names to tool enums
- `memory: BTreeMap<String, String>` — key-value store for the memory tools

`execute(name, args)` looks up the tool by name and dispatches to the
matching implementation method. Unknown tools return an error listing
all available tool names.

### Calculator

Parses `"<num> <op> <num>"` format. Supports `+`, `-`, `*`, `/` on `i64`
values. Division by zero returns an error instead of panicking.

### Memory Store / Recall

Simple key-value get/set backed by a `BTreeMap`. The memory store is the
agent's earliest form of persistent state — the precursor to the vector
store that will eventually provide long-term memory via semantic search.
