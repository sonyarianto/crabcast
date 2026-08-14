//! Media library: storage abstraction, tag scanning, waveform computation.

pub mod scan;
pub mod storage;

pub use scan::ScanResult;
pub use storage::{LocalStorage, Storage};
