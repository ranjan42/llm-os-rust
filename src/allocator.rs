//! Heap allocator — gives the kernel dynamic memory allocation.
//!
//! Maps a region of virtual address space for the heap and initializes
//! a linked-list allocator over it. This enables `alloc::Vec`, `alloc::String`,
//! `alloc::Box`, etc.
//!
//! In the LLM OS, the heap backs:
//! - The agent's context window (VecDeque<Message>)
//! - Token buffers
//! - Embedding vectors (Vec<f32>)
//! - Tool call arguments and results

use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Start address of the kernel heap.
pub const HEAP_START: usize = 0x_4444_4444_0000;
/// Size of the kernel heap: 1 MiB.
pub const HEAP_SIZE: usize = 1024 * 1024;

/// Initialize the kernel heap.
///
/// Maps `HEAP_SIZE` bytes of virtual memory starting at `HEAP_START`
/// to physical frames, then initializes the allocator over that region.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
