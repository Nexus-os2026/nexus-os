pub mod document;
pub mod economy;
pub mod engine;
pub mod extraction;
pub mod governance;
pub mod screen;
pub mod tauri_commands;
pub mod vision;

pub use engine::{PerceptionEngine, PerceptionError};
pub use governance::{PerceptionPolicy, PERCEPTION_CAPABILITY};
pub use tauri_commands::PerceptionState;
pub use vision::{
    ApiVisionProvider, ImageFormat, ImageSource, PerceptionResult, PerceptionTask, UIElement,
    UIElementType, VisionProvider, VisualInput,
};
