//! Focused implementation modules for the Foundation package.
//!
//! These modules preserve the former crate ownership boundaries without
//! making those boundaries independent crates.io release units.

pub mod analysis;
pub mod authoring;
pub mod codegen;
pub mod kir;
pub mod language_contracts;
pub mod model;
pub mod query_dsl;
pub mod runtime;
pub mod semantic_services;
pub mod session;
pub mod simulation_core;
pub mod views;
pub mod workspace;
