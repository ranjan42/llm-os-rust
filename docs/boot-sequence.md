# Boot Sequence

This document walks through what happens from the moment the machine powers on
to the point where the agent is running and waiting for input.

## Overview

The boot process has three phases:

1. **Bootloader** — the `bootloader` crate handles BIOS boot, enters long mode,
   sets up initial page tables, and jumps to `kernel_main`.
2. **Kernel init** — we set up the GDT, IDT, PIC, memory manager, and heap.
3. **Agent startup** — we create the Agent, load its system prompt, register
   tools, and enter the halt loop.

## Phase 1: Bootloader (external)

We use the `bootloader` crate (v0.9.23). It's a separate binary that gets
linked into the final disk image by the `bootimage` tool. Here's what it does:

1. BIOS loads the first sector of the disk (the bootloader's stage 1).
2. The bootloader switches from real mode (16-bit) to protected mode (32-bit)
   to long mode (64-bit).
3. It reads the kernel ELF binary from disk and loads it into memory.
4. It sets up an initial identity-mapped page table, plus a mapping of all
   physical memory at a configurable offset.
5. It builds a `BootInfo` struct containing the memory map and the physical
   memory offset.
6. It jumps to the kernel's entry point, passing `&BootInfo`.

We never write bootloader code ourselves. The `bootloader` crate handles all
of this. Our entry point is declared with the `entry_point!(kernel_main)` macro
in `src/main.rs`.

## Phase 2: Kernel Init

`kernel_main` receives a `&'static BootInfo` from the bootloader. It then
initializes the hardware, top to bottom:

### Step 1: GDT + TSS (`gdt.rs`)

```
llm_os::init()
  └── gdt::init()
        ├── Load the Global Descriptor Table
        ├── Set CS register to kernel code segment
        └── Load the Task State Segment (TSS)
```

The GDT is mostly vestigial in 64-bit mode (segmentation is disabled), but
the CPU still needs it for the TSS. The TSS provides the Interrupt Stack Table
(IST), which gives us a known-good stack to switch to when a double fault
occurs. Without this, a stack overflow causes a triple fault and the machine
reboots silently.

The double-fault stack is 20 KiB, statically allocated as a `[u8; 4096 * 5]`.

### Step 2: IDT (`interrupts.rs`)

```
llm_os::init()
  └── interrupts::init_idt()
        ├── Register breakpoint handler
        ├── Register double fault handler (uses IST entry 0)
        ├── Register page fault handler
        ├── Register timer interrupt handler (IRQ 0, mapped to vector 32)
        └── Register keyboard interrupt handler (IRQ 1, mapped to vector 33)
```

The IDT tells the CPU which function to call for each interrupt vector.
We register handlers for three CPU exceptions and two hardware interrupts.

### Step 3: PIC initialization

```
llm_os::init()
  └── PICS.lock().initialize()
        ├── Remap PIC1 to vectors 32-39
        └── Remap PIC2 to vectors 40-47
```

The 8259 PIC (Programmable Interrupt Controller) is the chip that routes
hardware interrupts to the CPU. By default, it maps IRQs 0-7 to interrupt
vectors 0-7, which collide with CPU exception vectors. We remap them
to vectors 32+ so they don't overlap.

After this, we call `x86_64::instructions::interrupts::enable()` to set the
CPU's interrupt flag, allowing hardware interrupts to fire.

### Step 4: Memory manager (`memory.rs`)

```
kernel_main()
  ├── memory::init(phys_mem_offset)
  │     └── Read CR3 to find the level-4 page table
  │         └── Create an OffsetPageTable using the bootloader's physical memory mapping
  │
  └── BootInfoFrameAllocator::init(&boot_info.memory_map)
        └── Wrap the bootloader's memory map into a frame allocator
            that hands out unused physical frames
```

The bootloader maps all of physical memory at a known virtual offset
(stored in `boot_info.physical_memory_offset`). We use this to access
the page tables that the CPU is already using, and we wrap them in an
`OffsetPageTable` for convenient manipulation.

The `BootInfoFrameAllocator` iterates over the memory map and yields
physical frames from regions marked as `Usable`.

### Step 5: Heap allocator (`allocator.rs`)

```
kernel_main()
  └── allocator::init_heap(&mut mapper, &mut frame_allocator)
        ├── Calculate the page range for the heap region
        │     Start: 0x4444_4444_0000
        │     Size:  1 MiB (256 pages)
        ├── For each page in the range:
        │     ├── Allocate a physical frame
        │     ├── Map the page to the frame (PRESENT + WRITABLE)
        │     └── Flush the TLB entry
        └── Initialize the linked-list allocator over the mapped region
```

After this, `alloc::Vec`, `alloc::String`, `alloc::Box`, and friends all
work. The heap is what makes the agent runtime possible — `VecDeque<Message>`,
`BTreeMap<String, String>`, and all the dynamic data structures live here.

## Phase 3: Agent Startup

```
kernel_main()
  └── Agent::new()
        ├── Create a ContextWindow with 4096-token capacity
        └── Create an empty ToolRegistry
  └── agent.boot()
        ├── Push the system prompt into the context window
        └── Register 4 built-in tools: calc, store, recall, echo
```

After the agent boots, the kernel prints a status message and enters the
halt loop (`hlt_loop`). From this point on, everything is interrupt-driven:
the timer fires periodically, and keyboard presses are captured and printed
to the VGA buffer. The agent's `process_input` method is the intended entry
point for feeding keyboard input to the agent, though right now the keyboard
handler just echoes characters directly.

## What the Halt Loop Does

```rust
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
```

The `HLT` instruction puts the CPU to sleep until the next interrupt fires.
This is much more power-efficient than a busy-wait spin loop. When an
interrupt arrives (timer tick, key press), the CPU wakes up, runs the
handler, and goes back to sleep.
