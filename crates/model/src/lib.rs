#![forbid(unsafe_code)]

pub mod midi;
pub mod project;
pub mod timeline;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub const DEFAULT_BLOCK_SIZE: usize = 128;
pub const MAX_STATE: usize = 64 * 1024 * 1024;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plugin {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub format: String,
}
