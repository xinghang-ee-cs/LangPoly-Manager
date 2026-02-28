//! MeetAI library crate.
//!
//! This crate exposes core modules so integration tests can exercise
//! runtime flows via stable module entry points.

/// CLI argument schema and command types.
pub mod cli;
/// Persistent app configuration and directory policy.
pub mod config;
/// Pip package and version management.
pub mod pip;
/// Python install, version, and venv management.
pub mod python;
/// One-command environment bootstrap flow.
pub mod quick_install;
/// Unified runtime command handlers.
pub mod runtime;
/// Shared utilities (downloader, executor, progress, validator).
pub mod utils;
