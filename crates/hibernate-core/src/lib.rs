//! `hibernate-core` is the project-aware scanning and cleanup engine behind
//! Project Hibernate. It has no UI dependencies: the desktop shell (Tauri) and
//! the command-line tool are both thin layers on top of this crate.
//!
//! The engine follows the product's guiding rule: **keep the project, remove
//! what can be rebuilt**. Nothing in this crate deletes source code. Every
//! removal goes through [`cleanup::safety`] first.

pub mod caches;
pub mod cleanup;
pub mod config;
pub mod export;
pub mod format;
pub mod fsx;
pub mod git;
pub mod history;
pub mod model;
pub mod projects;
pub mod scanner;
pub mod wake;

pub use cleanup::hibernate::{HibernateEvent, HibernatePlan, HibernateRequest, PlannedProject};
pub use cleanup::rules::{CleanupRule, RuleSet};
pub use config::{AppPaths, AppState, Disposition, Settings, Theme};
pub use history::{ArtifactOutcome, HistoryEntry, HistoryProject, HistoryStore};
pub use model::*;
pub use scanner::{scan, ScanEvent, ScanOptions, ScanResult, ScanSummary};
pub use wake::{WakePlan, WakeStep};
