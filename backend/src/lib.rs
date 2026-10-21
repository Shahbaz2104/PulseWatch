//! PulseWatch backend library root — shared by the `pulsewatch` binary and
//! the integration test suite in `backend/tests/`.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod routes;
pub mod services;
pub mod state;
pub mod validation;
