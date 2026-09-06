//! Project root detection: which directories are projects, what technology
//! they use, and which package manager wakes them.

pub mod detection;
pub mod stacks;

pub use detection::{detect, read_dir_lite, Detection, DirEntryLite};
