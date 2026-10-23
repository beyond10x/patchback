//! DESIGN.md § 6.1, § 12.1: RIFF and chunk sizes are checked before any buffer is sized. A
//! counting allocator records the largest single request made on this thread while a hostile
//! header is decoded; a declared size of `u32::MAX` must never reach the allocator.

mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use patchback_audio::{Truncation, WavError, decode};
use support::*;

struct Counting;

thread_local! {
    static LARGEST: Cell<usize> = const { Cell::new(0) };
}

fn note(size: usize) {
    let _ = LARGEST.try_with(|largest| largest.set(largest.get().max(size)));
}

// SAFETY: every call forwards to `System` unchanged; the only addition is a thread-local
// counter that never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The largest single allocation `decode` asks for on `bytes`, and its result.
fn largest_allocation(bytes: &[u8]) -> (usize, Result<usize, WavError>) {
    LARGEST.with(|largest| largest.set(0));
    let result = decode(bytes).map(|clip| clip.frame_count());
    (LARGEST.with(Cell::get), result)
}

const SMALL: usize = 4096;

#[test]
fn a_riff_size_of_u32_max_allocates_nothing_large() {
    let body = [
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &pcm16_data(&[0])),
    ]
    .concat();
    let (largest, result) = largest_allocation(&riff_declaring(u32::MAX, &body));
    assert!(matches!(
        result,
        Err(WavError::Truncated(Truncation::RiffSize { .. }))
    ));
    assert!(largest < SMALL, "largest allocation {largest} bytes");
}

#[test]
fn a_chunk_size_of_u32_max_allocates_nothing_large() {
    for id in [b"data", b"fmt ", b"LIST"] {
        let bytes = riff(&[chunk_header(id, u32::MAX)]);
        let (largest, result) = largest_allocation(&bytes);
        assert!(matches!(
            result,
            Err(WavError::Truncated(Truncation::Chunk { .. }))
        ));
        assert!(
            largest < SMALL,
            "{id:?}: largest allocation {largest} bytes"
        );
    }
}

#[test]
fn a_large_unknown_chunk_is_skipped_without_being_copied() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"junk", &vec![0; 1 << 20]),
        chunk(b"data", &pcm16_data(&[0, 1])),
    ]);
    let (largest, result) = largest_allocation(&bytes);
    assert_eq!(result, Ok(2));
    assert!(largest < SMALL, "largest allocation {largest} bytes");
}

#[test]
fn a_frame_count_past_the_bound_is_refused_before_the_samples_are_sized() {
    let frames = patchback_audio::MAX_FRAMES + 1;
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &vec![0; frames * 2]),
    ]);
    let (largest, result) = largest_allocation(&bytes);
    assert!(matches!(result, Err(WavError::InputTooLong { .. })));
    assert!(largest < SMALL, "largest allocation {largest} bytes");
}
