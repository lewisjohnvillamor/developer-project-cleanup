//! Everything that removes bytes. Every path goes through [`safety`] first.

pub mod hibernate;
pub mod quarantine;
pub mod remove;
pub mod rules;
pub mod safety;
pub mod trash;
