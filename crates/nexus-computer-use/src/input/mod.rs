pub mod backend;
pub mod keyboard;
pub mod mouse;
pub mod safety;

pub use backend::{detect_input_backend, InputBackend, InputBackendKind};
pub use keyboard::{KeyAction, KeyboardActionResult, KeyboardController};
pub use mouse::{MouseAction, MouseActionResult, MouseButton, MouseController, ScrollDirection};
pub use safety::InputSafetyGuard;
