//! MINION Application
//!
//! Main application entry point and Tauri integration.

pub mod app;
pub mod commands;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Application error: {0}")]
    App(String),
}

pub type Result<T> = std::result::Result<T, Error>;
