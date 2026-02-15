# Build System

This project has a more complex build pipeline than a typical Rust project
because we're compiling for a custom bare-metal target and producing a
bootable disk image. This document explains each piece.

## Files Involved

| File | Role |
|---|---|
| `Cargo.toml` | Dependencies and crate metadata |
| `rust-toolchain.toml` | Pins the Rust toolchain version |
| `x86_64-llm-os.json` | Custom compilation target definition |
| `.cargo/config.toml` | Cargo build configuration (target, build-std, runner) |

---

## `Cargo.toml`

Standard Rust project manifest. A few things worth noting:

**Bootloader dependency:**
```toml
bootloader = { path = "./bootloader", features = ["map_physical_memory"] }
```
- We vendor the `bootloader` crate (v0.9.23) locally in `./bootloader` instead of pulling from crates.io.
- **Why?** To ensure ABI stability and compatibility with modern Rust Nightly toolchains. The bootloader needs to be compiled with a target definition that exactly matches the kernel's expectations (specifically LLVM `data-layout`). By vendoring it, we can patch its internal `x86_64-bootloader.json` target file to match our kernel's data layout, preventing cryptic "stack overflow" or "kernel mapping failed" panics on boot.

**No-std crates:**
All dependencies are `no_std` compatible. The `lazy_static` crate uses
`features = ["spin_no_std"]` to swap out std mutexes for spinlocks.

**Panic behavior:**
```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```
We abort on panic instead of unwinding. Stack unwinding requires the
standard library's unwinder, which isn't available in `no_std`.

**Bootimage test configuration:**
```toml
[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
    "-serial", "stdio",
    "-display", "none"
]
```
These QEMU arguments are passed automatically when running tests:
- `isa-debug-exit` — a virtual device that lets the kernel tell QEMU
  to exit with a specific code (used by the test framework)
- `-serial stdio` — routes serial output to the terminal
- `-display none` — no GUI window needed for tests

---

## `rust-toolchain.toml`

```toml
[toolchain]
channel = "nightly-2025-02-01"
components = ["rust-src", "rustfmt", "clippy", "llvm-tools-preview"]
```

**Why nightly?** We use several unstable features:
- `custom_test_frameworks` — our own test runner (standard one needs std)
- `abi_x86_interrupt` — the calling convention for interrupt handlers
- `-Zbuild-std` — recompiling `core` and `alloc` for our custom target

**Why pinned to 2025-02-01?** Later nightlies introduced breaking changes:
- `compiler_builtins` now embeds `libm`, which generates code using SSE
  registers. Since our target disables SSE (bare metal kernels shouldn't
  use SSE without saving/restoring the SSE state on context switches),
  this causes "SSE register return with SSE disabled" errors.
- The 2025-02-01 nightly predates this regression.

**Components:**
- `rust-src` — source code for `core`, `alloc`, `compiler_builtins`. Needed
  by `build-std` to recompile them for our custom target.
- `llvm-tools-preview` — LLVM utilities (`llvm-objcopy`, `llvm-size`, etc.)
  used by `bootimage` to assemble the disk image.

---

## `x86_64-llm-os.json` — Custom Target

This file defines a custom compilation target for the Rust compiler. We
can't use any built-in target because we need specific bare-metal settings.

```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "pre-link-args": {
        "ld.lld": [
            "--script=src/linker.ld"
        ]
    },
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "-mmx,-sse,+soft-float"
}
```

**Key fields explained:**

- **`llvm-target`**: tells LLVM which backend to use. `x86_64-unknown-none`
  is a generic freestanding x86_64 target.

- **`data-layout`**: LLVM's data layout string. Must match the LLVM version
  in the toolchain exactly. We maintain ours to match `nightly-2025-02-01`.

- **`pre-link-args`**:
  - `--script=src/linker.ld`: Tells the linker to use our custom linker script.
  This is **CRITICAL**. Without it, the linker uses default section alignment
  (often 2MB or variable), which can cause ELF segments to overlap on physical
  pages in a way that confuses the bootloader. If you see panics like
  `Mapping(PageAlreadyMapped(PhysFrame...))`, this is the fix. The script
  forces 4KiB page alignment for all sections.

- **`linker-flavor` / `linker`**: Use LLVM's `lld` linker via `rust-lld`.

- **`panic-strategy: "abort"`**: don't try to unwind on panic.

- **`disable-redzone: true`**: mandatory for kernel code to prevent
  interrupts from corrupting the stack.

- **`features: "-mmx,-sse,+soft-float"`**: disable SIMD to avoid
  complex context switching requirements.

---

## `src/linker.ld` — Custom Linker Script

We use a explicit linker script to control the kernel's memory layout.

```ld
ENTRY(_start)

SECTIONS {
    . = 0x200000; /* Load kernel at 2MB */

    .text : {
        *(.text .text.*)
    }

    . = ALIGN(4096);
    .rodata : {
        *(.rodata .rodata.*)
    }

    /* ... .data and .bss similarly aligned ... */
}
```

**Why is this needed?**
The `bootloader` crate (v0.9.x) maps the kernel by iterating over ELF segments.
If the linker aggressively packs sections (e.g., putting `.text` end and
`.rodata` start on the same 4KiB page), the bootloader sees two different
segments mapping to the same physical frame. It tries to map the frame twice
(once Read-Exec, once Read-Only), which causes a `PageAlreadyMapped` panic.

By forcing `ALIGN(4096)` between sections, we ensure every section starts on
a fresh page, preventing segment overlap and keeping the bootloader happy.

---

## `.cargo/config.toml`

```toml
[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
build-std-features = ["compiler-builtins-mem"]

[build]
target = "x86_64-llm-os.json"

[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

**`build-std`**: recompile these standard library crates from source for
our custom target. Normally the standard library is precompiled, but only
for known targets. Our custom target has no precompiled libraries, so we
must build them ourselves.

- `core` — fundamental types, traits, and intrinsics (no heap, no OS)
- `compiler_builtins` — software implementations of operations the compiler
  assumes exist (memcpy, memset, integer division, etc.)
- `alloc` — heap allocation types (Vec, String, Box) — requires a
  `#[global_allocator]`

**`build-std-features`**: `compiler-builtins-mem` enables software
implementations of `memcpy`, `memset`, and `memcmp` in `compiler_builtins`.
Without this, you get linker errors because no libc provides these.

**`target`**: sets the default target so you don't have to pass
`--target x86_64-llm-os.json` on every `cargo` command.

**`runner`**: when you run `cargo run`, it invokes `bootimage runner` instead
of trying to execute the kernel binary directly (which would fail since
it's not a native executable).

---

## Build Pipeline

Here's what actually happens when you run `cargo bootimage`:

```
1. cargo builds the kernel (src/main.rs → ELF binary)
   ├── Recompiles core, alloc, compiler_builtins for x86_64-llm-os
   ├── Compiles all dependencies (bootloader, x86_64, spin, etc.)
   └── Links into target/x86_64-llm-os/debug/llm-os

2. bootimage builds the bootloader (bootloader crate's src/main.rs)
   ├── Recompiles core for x86_64-bootloader (the bootloader's own target)
   ├── Embeds the kernel ELF as a binary blob using llvm-objcopy
   └── Links into target/bootimage/bootloader/x86_64-bootloader.json/release/bootloader

3. bootimage creates the disk image
   ├── Concatenates bootloader + kernel into a flat binary
   └── Writes target/x86_64-llm-os/debug/bootimage-llm-os.bin
```

The result is a raw disk image that BIOS can boot directly. QEMU treats
it as a floppy or hard disk and boots from sector 0.
