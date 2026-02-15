//! LLM OS — An AI-First Operating System in Rust
//!
//! Inspired by Andrej Karpathy's "LLM OS" concept, this kernel inverts the
//! traditional OS architecture: instead of serving a human user, the kernel
//! serves a single AI Agent that *is* the userland.
//!
//! Architecture:
//!   CPU       → LLM (processes tokens instead of machine instructions)
//!   RAM       → Context Window (working memory for the current reasoning chain)
//!   Disk      → Vector Store / RAG (long-term memory via semantic retrieval)
//!   Syscalls  → Tool Invocations (calculator, web search, code execution)
//!   Scheduler → Orchestrator (multiplexes inference cycles across tasks)

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(llm_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::string::String;
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use llm_os::{println, serial_println};

entry_point!(kernel_main);

/// The kernel entry point — boots the hardware and hands off to the agent.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("==================================");
    println!("  LLM OS v0.1.0");
    println!("  An AI-First Operating System");
    println!("==================================");
    serial_println!("[kernel] Booting LLM OS...");

    // Phase 1: Initialize hardware
    llm_os::init();
    serial_println!("[kernel] GDT, IDT, PIC initialized");

    // Phase 2: Set up memory management
    use llm_os::memory;
    use x86_64::VirtAddr;

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Initialize the heap allocator
    llm_os::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
    serial_println!("[kernel] Heap allocator initialized");

    // Phase 3: Initialize the Agent runtime
    println!();
    println!("Hardware initialized.");
    println!("Starting Agent runtime...");
    serial_println!("[kernel] Starting Agent runtime...");

    let mut agent = llm_os::agent::Agent::new();
    agent.boot();

    println!();
    println!("Agent is alive. Entering run loop.");
    serial_println!("[kernel] Agent run loop started");

    #[cfg(test)]
    test_main();

    // Main kernel loop — the agent processes input from interrupts
    llm_os::hlt_loop();
}

/// Panic handler for normal (non-test) mode.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("KERNEL PANIC: {}", info);
    serial_println!("KERNEL PANIC: {}", info);
    llm_os::hlt_loop();
}

/// Panic handler for test mode.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    llm_os::test_panic_handler(info)
}
