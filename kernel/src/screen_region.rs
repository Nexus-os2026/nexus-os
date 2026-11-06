//! Shared screen-region data, extracted unchanged from computer control.

use serde::{Deserialize, Serialize};

/// A rectangular screen region for targeted capture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
