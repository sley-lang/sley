//! Sley 2 SH2 self-hosted toolchain.
//!
//! - [`seed`]: the C0 seed assembler that constructs typed SSMC1 graphs.
//! - [`native`]: the C0-side native harness (reference lowering and
//!   execution through the existing `sley-vm` authority).

pub mod native;
pub mod seed;
