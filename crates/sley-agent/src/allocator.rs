//! The `sley-agent` binary's allocator: a bounded size-class cache over the
//! system allocator (ADR-0052).
//!
//! The release binary links musl statically. musl's allocator hands freed
//! memory back to the kernel eagerly, so a batch that allocates and frees the
//! same sizes for every input pays for page faults and system time each
//! time: about half the CPU of a 1,000-input `call --batch`. This allocator
//! keeps freed blocks of up to 64 KiB, with alignment up to 16, per thread
//! and per power-of-two size class, and hands them out again.
//!
//! - Every cached block was allocated from the system allocator with its
//!   class's layout. Larger or more strictly aligned requests go straight to
//!   the system allocator.
//! - A thread keeps at most 2 MiB and at most 1,024 blocks per class. A
//!   block freed beyond that goes back to the system allocator.
//! - When the system allocator fails, the thread's cached blocks go back to
//!   it and the request is retried once.
//! - A thread's cached blocks are not returned when it exits: they stay
//!   allocated, within the same bounds. The binary runs on one thread.
//! - The lists hold addresses, never links written into freed blocks. Every
//!   cached block's provenance is exposed when it is first allocated, and a
//!   reused block's pointer is rebuilt from that exposed provenance, so a
//!   block is handed out with rights over its whole extent whichever pointer
//!   freed it.
//!
//! This module is the workspace's one `unsafe` exception (ADR-0052). It is a
//! module of the `sley-agent` binary target, not of the library: the library
//! forbids `unsafe_code`, and no crate can depend on a binary. The workbench
//! integration tests include it by path so that the suite runs on it.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ptr;

/// Size classes 16 B, 32 B, ..., 64 KiB.
const CLASSES: usize = 13;
const MIN_SHIFT: usize = 4;
/// The largest cached block.
const MAX_CACHED: usize = 1 << (MIN_SHIFT + CLASSES - 1);
/// Every cached block has this alignment, the largest a cached request may ask for.
const CACHED_ALIGN: usize = 16;
/// Blocks a thread keeps per class, at most.
const SLOTS: usize = 1024;
/// Bytes a thread keeps per class, at most.
const CLASS_BYTES: usize = 2 << 20;

/// One thread's cached blocks: per class, a stack of block addresses.
struct Lists {
    slots: [[Cell<usize>; SLOTS]; CLASSES],
    len: [Cell<usize>; CLASSES],
}

thread_local! {
    /// Constant initialization and no destructor: using it never allocates,
    /// and it is usable for the whole life of its thread.
    static LISTS: Lists = const {
        Lists {
            slots: [const { [const { Cell::new(0) }; SLOTS] }; CLASSES],
            len: [const { Cell::new(0) }; CLASSES],
        }
    };
}

/// The allocator. See the module documentation.
pub struct SizeClassCache;

impl SizeClassCache {
    fn extent(layout: Layout) -> usize {
        Self::class(layout).map_or(layout.size(), |class| Self::block(class).size())
    }
    /// The class that serves `layout`, or `None` for the system allocator.
    /// A pure function of the layout, so `dealloc` (which receives the
    /// layout `alloc` was given) always finds the class `alloc` used.
    const fn class(layout: Layout) -> Option<usize> {
        if layout.align() > CACHED_ALIGN || layout.size() > MAX_CACHED {
            return None;
        }
        let size = if layout.size() < (1 << MIN_SHIFT) {
            1 << MIN_SHIFT
        } else {
            layout.size()
        };
        // size <= MAX_CACHED, so the class is below CLASSES.
        Some(size.next_power_of_two().trailing_zeros() as usize - MIN_SHIFT)
    }

    /// The layout every block of `class` is allocated with.
    const fn block(class: usize) -> Layout {
        match Layout::from_size_align(1 << (class + MIN_SHIFT), CACHED_ALIGN) {
            Ok(layout) => layout,
            Err(_) => panic!("size classes are valid layouts"),
        }
    }

    /// How many blocks of `class` a thread keeps.
    const fn depth(class: usize) -> usize {
        let blocks = CLASS_BYTES >> (class + MIN_SHIFT);
        if blocks < SLOTS { blocks } else { SLOTS }
    }

    /// Takes a cached block of `class` from this thread, or null.
    fn pop(class: usize) -> *mut u8 {
        LISTS
            .try_with(|lists| {
                let len = lists.len[class].get();
                if len == 0 {
                    return ptr::null_mut();
                }
                lists.len[class].set(len - 1);
                ptr::with_exposed_provenance_mut(lists.slots[class][len - 1].get())
            })
            .unwrap_or(ptr::null_mut())
    }

    /// Caches a freed block of `class` on this thread; false when the class
    /// is full (or the lists are unavailable), and the caller releases it.
    fn push(class: usize, block: *mut u8) -> bool {
        LISTS
            .try_with(|lists| {
                let len = lists.len[class].get();
                if len >= Self::depth(class) {
                    return false;
                }
                lists.slots[class][len].set(block.addr());
                lists.len[class].set(len + 1);
                true
            })
            .unwrap_or(false)
    }

    /// Returns every block this thread caches to the system allocator;
    /// whether there was any.
    fn drain() -> bool {
        LISTS
            .try_with(|lists| {
                let mut released = false;
                for class in 0..CLASSES {
                    while let len @ 1.. = lists.len[class].get() {
                        lists.len[class].set(len - 1);
                        let block =
                            ptr::with_exposed_provenance_mut(lists.slots[class][len - 1].get());
                        // SAFETY: a cached block, allocated from the system
                        // allocator with this class's layout and used by no one.
                        unsafe { System.dealloc(block, Self::block(class)) };
                        released = true;
                    }
                }
                released
            })
            .unwrap_or(false)
    }

    /// A new block of `class` from the system allocator (zeroed on request),
    /// retried once after a drain; its provenance is exposed for reuse.
    fn fresh(class: usize, zeroed: bool) -> *mut u8 {
        let layout = Self::block(class);
        let allocate = || {
            // SAFETY: a class layout has a non-zero size.
            unsafe {
                if zeroed {
                    System.alloc_zeroed(layout)
                } else {
                    System.alloc(layout)
                }
            }
        };
        let mut block = allocate();
        if block.is_null() && Self::drain() {
            block = allocate();
        }
        if !block.is_null() {
            let _ = block.expose_provenance();
        }
        block
    }

    /// `call` on the system allocator, retried once after a drain.
    fn system(call: impl Fn() -> *mut u8) -> *mut u8 {
        let block = call();
        if block.is_null() && Self::drain() {
            call()
        } else {
            block
        }
    }
}

// SAFETY: every block handed out is either a block of the request's class (at
// least the requested size, aligned to 16, which is at least the requested
// alignment, with the provenance of the whole block) or the system
// allocator's own answer for the request's exact layout. A block is cached
// only after `dealloc`, and it leaves the cache before it is handed out
// again. The caches are per thread, so no two threads touch one.
unsafe impl GlobalAlloc for SizeClassCache {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let block = match Self::class(layout) {
            Some(class) => {
                let cached = Self::pop(class);
                if cached.is_null() {
                    Self::fresh(class, false)
                } else {
                    cached
                }
            }
            // SAFETY: the caller's layout, unchanged.
            None => Self::system(|| unsafe { System.alloc(layout) }),
        };
        if !block.is_null() {
            sley_agent::residual::frontier::heap::allocated(Self::extent(layout));
        }
        block
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let block = match Self::class(layout) {
            Some(class) => {
                let cached = Self::pop(class);
                if cached.is_null() {
                    let block = Self::fresh(class, true);
                    if !block.is_null() {
                        sley_agent::residual::frontier::heap::allocated(Self::extent(layout));
                    }
                    return block;
                }
                // SAFETY: the block holds at least `layout.size()` bytes.
                unsafe { cached.write_bytes(0, layout.size()) };
                cached
            }
            // SAFETY: the caller's layout, unchanged.
            None => Self::system(|| unsafe { System.alloc_zeroed(layout) }),
        };
        if !block.is_null() {
            sley_agent::residual::frontier::heap::allocated(Self::extent(layout));
        }
        block
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        sley_agent::residual::frontier::heap::deallocated(Self::extent(layout));
        match Self::class(layout) {
            Some(class) => {
                if !Self::push(class, block) {
                    // SAFETY: `alloc` served this layout from `class`, so the
                    // block came from the system allocator with the class
                    // layout; the rebuilt pointer has the whole block's
                    // provenance.
                    unsafe {
                        System.dealloc(
                            ptr::with_exposed_provenance_mut(block.addr()),
                            Self::block(class),
                        );
                    }
                }
            }
            // SAFETY: `alloc` got this block from the system allocator with
            // this layout.
            None => unsafe { System.dealloc(block, layout) },
        }
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees `new_size`, rounded up to the
        // alignment, does not overflow `isize`.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (Self::class(layout), Self::class(new_layout)) {
            // The block already has room for the new size; the rebuilt
            // pointer has the whole block's provenance.
            (Some(old), Some(new)) if old == new => ptr::with_exposed_provenance_mut(block.addr()),
            // SAFETY: the system allocator's block, resized under its own rules.
            (None, None) => {
                sley_agent::residual::frontier::heap::reallocating(new_size);
                let resized = Self::system(|| unsafe { System.realloc(block, layout, new_size) });
                if !resized.is_null() {
                    if new_size >= layout.size() {
                        sley_agent::residual::frontier::heap::allocated(new_size - layout.size());
                    } else {
                        sley_agent::residual::frontier::heap::deallocated(layout.size() - new_size);
                    }
                }
                resized
            }
            _ => {
                // SAFETY: a valid non-zero layout (the caller's alignment and size).
                let moved = unsafe { self.alloc(new_layout) };
                if !moved.is_null() {
                    // SAFETY: both blocks hold at least the smaller size and do
                    // not overlap; the old block is then released with its
                    // own layout.
                    unsafe {
                        ptr::copy_nonoverlapping(block, moved, layout.size().min(new_size));
                        self.dealloc(block, layout);
                    }
                }
                moved
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CLASS_BYTES, CLASSES, MAX_CACHED, SLOTS, SizeClassCache};
    use sley_agent::residual::frontier::{Budget, heap};
    use std::alloc::{GlobalAlloc, Layout};
    use std::time::Duration;

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    /// Local-allocation lower bounds require a process with no unrelated live
    /// baseline being freed by parallel tests. Preserve the real global counter
    /// and all assertions by selecting this same test in a fresh process.
    fn isolated_heap_test(name: &str) -> bool {
        const CHILD_TEST: &str = "SLEY_ALLOCATOR_HEAP_TEST_CASE";
        // Miri does not support spawning this native test executable. Its
        // allocator checks run with --test-threads=1 instead.
        if cfg!(miri) {
            return false;
        }
        let selected = format!("allocator::tests::{name}");
        if std::env::var(CHILD_TEST).ok().as_deref() == Some(selected.as_str()) {
            return false;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selected, "--test-threads=1", "--nocapture"])
            .env(CHILD_TEST, &selected)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("test result: ok. 1 passed; 0 failed"),
            "isolated {selected}: {}\n{stdout}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        true
    }

    #[test]
    fn heap_budget_retains_a_released_transient_peak_and_exhaustion_is_sticky() {
        if isolated_heap_test(
            "heap_budget_retains_a_released_transient_peak_and_exhaustion_is_sticky",
        ) {
            return;
        }
        heap::enable_current_thread();
        let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, 1 << 20);
        let cache = SizeClassCache;
        // SAFETY: the non-null block is used within its layout and released
        // exactly once with that same layout. The allocator observer, not RSS,
        // is the subject of this test.
        unsafe {
            let request = layout(2 << 20, 8);
            let block = cache.alloc_zeroed(request);
            assert!(!block.is_null());
            block.write(42);
            assert_eq!(block.read(), 42);
            cache.dealloc(block, request);
        }
        assert!(
            budget
                .checkpoint()
                .unwrap_err()
                .detail()
                .contains("observed planner heap")
        );
        assert!(budget.checkpoint().is_err());
        let usage = budget.usage();
        assert_eq!(usage["allocator_heap"]["accounting_valid"], true);
        assert_eq!(usage["allocator_heap"]["exhausted"], true);
        assert!(
            usage["allocator_heap"]["peak_additional_bytes"]
                .as_u64()
                .unwrap()
                >= 2 << 20
        );
        assert_eq!(
            usage["aggregate_memory_enforcement"],
            "allocator_peak_checked_at_budget_checkpoints"
        );
    }

    #[test]
    fn nested_heap_observations_do_not_erase_the_outer_peak() {
        if isolated_heap_test("nested_heap_observations_do_not_erase_the_outer_peak") {
            return;
        }
        heap::enable_current_thread();
        let mut outer = Budget::limited_with_memory(Duration::from_secs(2), 100_000, 1 << 20);
        let mut inner = Budget::limited_with_memory(Duration::from_secs(2), 100_000, 64 << 20);
        let cache = SizeClassCache;
        // SAFETY: successful allocation and exactly matching release.
        unsafe {
            let request = layout(2 << 20, 32);
            let block = cache.alloc(request);
            assert!(!block.is_null());
            cache.dealloc(block, request);
        }
        inner.checkpoint().unwrap();
        drop(inner);
        assert!(outer.checkpoint().is_err());
    }

    #[test]
    fn system_reallocation_accounts_for_source_and_destination_scratch() {
        if isolated_heap_test("system_reallocation_accounts_for_source_and_destination_scratch") {
            return;
        }
        heap::enable_current_thread();
        let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, 5 << 18);
        let cache = SizeClassCache;
        // SAFETY: both layouts exceed the cache, the successful realloc
        // preserves the initialized byte, and the final layout matches release.
        unsafe {
            let old = layout(1 << 19, 8);
            let block = cache.alloc(old);
            assert!(!block.is_null());
            block.write(7);
            let resized = cache.realloc(block, old, 1 << 20);
            assert!(!resized.is_null());
            assert_eq!(resized.read(), 7);
            cache.dealloc(resized, layout(1 << 20, 8));
        }
        assert!(budget.checkpoint().is_err());
        let usage = budget.usage();
        assert!(
            usage["allocator_heap"]["peak_additional_bytes"]
                .as_u64()
                .unwrap()
                >= 3 << 19,
            "{usage}"
        );
    }

    #[test]
    fn heap_watch_capacity_fails_closed_and_cross_thread_drop_releases_scope() {
        heap::enable_current_thread();
        let mut scopes: Vec<_> = (0..32).map(|_| Budget::default()).collect();
        let mut unavailable = Budget::default();
        assert!(
            unavailable
                .checkpoint()
                .unwrap_err()
                .detail()
                .contains("observation unavailable")
        );
        let scope = scopes.pop().unwrap();
        std::thread::spawn(move || drop(scope)).join().unwrap();
        let mut replacement = Budget::default();
        replacement.checkpoint().unwrap();
        assert_eq!(
            replacement.usage()["allocator_heap"]["accounting_valid"],
            true
        );
        assert!(unavailable.checkpoint().is_err());
    }

    #[test]
    fn classes_cover_every_cached_size_within_their_bounds() {
        assert_eq!(SizeClassCache::class(layout(1, 1)), Some(0));
        assert_eq!(SizeClassCache::class(layout(16, 16)), Some(0));
        assert_eq!(SizeClassCache::class(layout(17, 8)), Some(1));
        assert_eq!(
            SizeClassCache::class(layout(MAX_CACHED, 16)),
            Some(CLASSES - 1)
        );
        assert_eq!(SizeClassCache::class(layout(MAX_CACHED + 1, 1)), None);
        assert_eq!(SizeClassCache::class(layout(8, 32)), None);
        for class in 0..CLASSES {
            let depth = SizeClassCache::depth(class);
            assert!((1..=SLOTS).contains(&depth));
            assert!(depth * SizeClassCache::block(class).size() <= CLASS_BYTES);
        }
    }

    #[test]
    fn a_freed_block_is_reused_for_its_class() {
        let cache = SizeClassCache;
        unsafe {
            let first = cache.alloc(layout(100, 8));
            first.write_bytes(0xAB, 100);
            cache.dealloc(first, layout(100, 8));
            // 65..=128 bytes share a class: the same block comes back, and
            // all 128 bytes are writable through the new pointer.
            let second = cache.alloc(layout(128, 16));
            assert_eq!(first.addr(), second.addr());
            assert_eq!(second.align_offset(16), 0);
            second.write_bytes(0xCD, 128);
            // Another class never takes it.
            let other = cache.alloc(layout(129, 8));
            assert_ne!(other.addr(), second.addr());
            cache.dealloc(second, layout(128, 16));
            cache.dealloc(other, layout(129, 8));
        }
    }

    #[test]
    fn a_full_class_releases_blocks_to_the_system() {
        let cache = SizeClassCache;
        let class = CLASSES - 1;
        let request = layout(MAX_CACHED, 16);
        let depth = SizeClassCache::depth(class);
        unsafe {
            let blocks: Vec<*mut u8> = (0..depth + 5).map(|_| cache.alloc(request)).collect();
            for block in &blocks {
                cache.dealloc(*block, request);
            }
            // Only `depth` of them were kept: that many come back cached.
            let again: Vec<*mut u8> = (0..depth).map(|_| cache.alloc(request)).collect();
            let kept = again
                .iter()
                .filter(|block| blocks.iter().any(|old| old.addr() == block.addr()))
                .count();
            assert_eq!(kept, depth);
            for block in again {
                cache.dealloc(block, request);
            }
        }
    }

    #[test]
    fn zeroed_blocks_are_zero_after_reuse() {
        let cache = SizeClassCache;
        unsafe {
            let dirty = cache.alloc(layout(64, 8));
            dirty.write_bytes(0xFF, 64);
            cache.dealloc(dirty, layout(64, 8));
            let clean = cache.alloc_zeroed(layout(64, 8));
            assert_eq!(clean.addr(), dirty.addr());
            assert!(
                std::slice::from_raw_parts(clean, 64)
                    .iter()
                    .all(|&byte| byte == 0)
            );
            cache.dealloc(clean, layout(64, 8));
            let large = cache.alloc_zeroed(layout(MAX_CACHED * 4, 8));
            assert!(
                std::slice::from_raw_parts(large, MAX_CACHED * 4)
                    .iter()
                    .all(|&byte| byte == 0)
            );
            cache.dealloc(large, layout(MAX_CACHED * 4, 8));
        }
    }

    #[test]
    fn realloc_keeps_the_contents_across_classes() {
        let cache = SizeClassCache;
        unsafe {
            let mut block = cache.alloc(layout(20, 4));
            for index in 0..20 {
                block.add(index).write(u8::try_from(index).unwrap());
            }
            // Within the class (17..=32): the same block, writable to 32.
            let same = cache.realloc(block, layout(20, 4), 32);
            assert_eq!(same.addr(), block.addr());
            same.add(31).write(7);
            block = same;
            // Up through the classes and past the cache, then back down.
            let mut size = 32;
            for new_size in [33, 1000, 70_000, 200_000, 40] {
                block = cache.realloc(block, layout(size, 4), new_size);
                assert!(!block.is_null());
                size = new_size;
            }
            for index in 0..20 {
                assert_eq!(block.add(index).read(), u8::try_from(index).unwrap());
            }
            cache.dealloc(block, layout(size, 4));
        }
    }

    #[test]
    fn large_and_overaligned_requests_go_to_the_system() {
        let cache = SizeClassCache;
        unsafe {
            for request in [
                layout(MAX_CACHED + 1, 8),
                layout(64, 64),
                layout(4096, 4096),
            ] {
                let block = cache.alloc(request);
                assert!(!block.is_null());
                assert_eq!(block.align_offset(request.align()), 0);
                block.write_bytes(1, request.size());
                cache.dealloc(block, request);
            }
        }
    }

    #[test]
    fn many_blocks_of_every_class_stay_distinct() {
        let cache = SizeClassCache;
        let mut live = Vec::new();
        unsafe {
            for round in 0..3 {
                for class in 0..CLASSES {
                    let request = layout((1 << (class + 4)) - round, 8);
                    let block = cache.alloc(request);
                    block.write_bytes(u8::try_from(class).unwrap(), request.size());
                    live.push((block, request));
                }
            }
            let mut addresses: Vec<usize> = live.iter().map(|(block, _)| block.addr()).collect();
            addresses.sort_unstable();
            addresses.dedup();
            assert_eq!(addresses.len(), live.len());
            for (block, request) in &live {
                let class = SizeClassCache::class(*request).unwrap();
                let bytes = std::slice::from_raw_parts(*block, request.size());
                assert!(bytes.iter().all(|&byte| usize::from(byte) == class));
            }
            for (block, request) in live {
                cache.dealloc(block, request);
            }
        }
    }

    #[test]
    fn blocks_move_between_threads_within_the_bounds() {
        // Blocks allocated on one thread are freed on another and reused
        // there; every block keeps its own contents throughout.
        let handles: Vec<_> = (0..8_u8)
            .map(|tag| {
                std::thread::spawn(move || {
                    let cache = SizeClassCache;
                    let mut owned = Vec::new();
                    unsafe {
                        for size in [24, 200, 3000, 40_000] {
                            let block = cache.alloc(layout(size, 8));
                            block.write_bytes(tag, size);
                            owned.push((block.addr(), size));
                        }
                    }
                    owned
                })
            })
            .collect();
        let blocks: Vec<(usize, usize, u8)> = handles
            .into_iter()
            .enumerate()
            .flat_map(|(tag, handle)| {
                let tag = u8::try_from(tag).unwrap();
                handle
                    .join()
                    .unwrap()
                    .into_iter()
                    .map(move |(block, size)| (block, size, tag))
            })
            .collect();
        let cache = SizeClassCache;
        unsafe {
            for (block, size, tag) in &blocks {
                let bytes = std::slice::from_raw_parts(
                    std::ptr::with_exposed_provenance::<u8>(*block),
                    *size,
                );
                assert!(bytes.iter().all(|byte| byte == tag));
            }
            for (block, size, _) in &blocks {
                cache.dealloc(
                    std::ptr::with_exposed_provenance_mut(*block),
                    layout(*size, 8),
                );
            }
            // Freed here, they are reused here.
            let again = cache.alloc(layout(40_000, 8));
            assert!(blocks.iter().any(|(block, _, _)| *block == again.addr()));
            cache.dealloc(again, layout(40_000, 8));
        }
    }
}
