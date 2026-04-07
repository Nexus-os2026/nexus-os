pub mod backend;
pub mod screenshot;

pub use backend::{BackendKind, CaptureBackend, DisplayServer};
pub use screenshot::{CaptureRegion, Screenshot, ScreenshotOptions};
