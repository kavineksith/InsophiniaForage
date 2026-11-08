//! Tauri command layer. Each function here is callable from the frontend
//! via `invoke('command_name', { args })`. Commands are thin wrappers
//! around the core modules — they handle Tauri-specific concerns
//! (State, events, async runtime) and delegate the real work to `core::*`.

pub mod binaries;
pub mod composer;
pub mod databases;
pub mod detection;
pub mod logs;
pub mod package_manager;
pub mod php_versions;
pub mod projects;
pub mod services;
pub mod settings;
pub mod source_build;
pub mod wizard;
pub mod wordpress;
