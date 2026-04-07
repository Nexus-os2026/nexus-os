use thiserror::Error;

#[derive(Error, Debug)]
pub enum ComputerUseError {
    #[error("Screen capture failed: {0}")]
    CaptureError(String),

    #[error("No capture backend available. Install grim (Wayland) or scrot (X11)")]
    NoBackendAvailable,

    #[error("Capability not granted: {capability}")]
    CapabilityDenied { capability: String },

    #[error("Display server not detected")]
    NoDisplayServer,

    #[error("Image processing error: {0}")]
    ImageError(String),

    #[error("Capture timed out after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("Invalid capture region: {reason}")]
    InvalidRegion { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Input command failed: {0}")]
    InputError(String),

    #[error("No input backend available. Install xdotool (X11) or ydotool (Wayland)")]
    NoInputBackendAvailable,

    #[error("Coordinates out of bounds: ({x}, {y}) exceeds screen {width}x{height}")]
    CoordinatesOutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },

    #[error("Blocked key combination: {combo} (requires SystemKeyCombos capability)")]
    BlockedKeyCombination { combo: String },

    #[error("Rate limit exceeded: max {max_per_second} actions per second")]
    RateLimitExceeded { max_per_second: u32 },

    #[error("Dead man's switch: {actions_without_screenshot} actions without screenshot (max {max_actions})")]
    DeadManSwitch {
        actions_without_screenshot: u32,
        max_actions: u32,
    },

    #[error("Input timed out after {seconds}s")]
    InputTimeout { seconds: u64 },

    #[error("Vision analysis failed: {0}")]
    VisionError(String),

    #[error("Action plan parse failed: {0}")]
    ActionPlanParseError(String),

    #[error("Agent loop exceeded max steps: {max_steps}")]
    MaxStepsExceeded { max_steps: u32 },

    #[error("Agent task completed: {summary}")]
    TaskComplete { summary: String },

    #[error("Agent aborted by user")]
    UserAborted,

    #[error("Low confidence ({confidence:.2}) below threshold ({threshold:.2})")]
    LowConfidence { confidence: f64, threshold: f64 },
}
