//! Crabcast API server library root. The binary (`main.rs`) is a thin
//! shell around these modules; exposing them as a lib lets the criterion
//! benches (`benches/`) link against the real code.

pub mod analytics;
pub mod api;
pub mod auth;
pub mod control;
pub mod db;
pub mod lua;
pub mod media;
pub mod notify;
pub mod stations;
