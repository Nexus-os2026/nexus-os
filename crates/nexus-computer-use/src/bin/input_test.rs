use nexus_computer_use::capability::check_system_requirements;
use nexus_computer_use::input::backend::{detect_input_backend, get_display_geometry};
use nexus_computer_use::input::keyboard::{KeyAction, KeyboardController};
use nexus_computer_use::input::mouse::{
    MouseAction, MouseButton, MouseController, ScrollDirection,
};
use nexus_computer_use::input::safety::InputSafetyGuard;

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  nx-input click <x> <y> [left|right|middle]");
    eprintln!("  nx-input dblclick <x> <y> [left|right|middle]");
    eprintln!("  nx-input move <x> <y>");
    eprintln!("  nx-input scroll <x> <y> <up|down|left|right> <amount>");
    eprintln!("  nx-input drag <start_x> <start_y> <end_x> <end_y>");
    eprintln!("  nx-input type \"<text>\"");
    eprintln!("  nx-input key <keyname>");
    eprintln!("  nx-input combo <key1+key2+...>");
    eprintln!("  nx-input position");
    eprintln!("  nx-input status");
}

fn parse_button(s: &str) -> Option<MouseButton> {
    match s.to_lowercase().as_str() {
        "left" | "1" => Some(MouseButton::Left),
        "right" | "3" => Some(MouseButton::Right),
        "middle" | "2" => Some(MouseButton::Middle),
        _ => None,
    }
}

fn parse_scroll_dir(s: &str) -> Option<ScrollDirection> {
    match s.to_lowercase().as_str() {
        "up" => Some(ScrollDirection::Up),
        "down" => Some(ScrollDirection::Down),
        "left" => Some(ScrollDirection::Left),
        "right" => Some(ScrollDirection::Right),
        _ => None,
    }
}

fn parse_u32(s: &str, name: &str) -> Result<u32, String> {
    s.parse::<u32>().map_err(|e| format!("Invalid {name}: {e}"))
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("=== Nexus Computer Use -- Input Control ===\n");
        print_usage();
        return Err("No command specified".into());
    }

    let command = args[1].as_str();

    if command == "status" {
        let req = check_system_requirements();
        println!("=== Input Control Status ===");
        println!(
            "  Display server: {}",
            req.display_server.as_deref().unwrap_or("none")
        );
        println!(
            "  Input tool:     {}",
            req.input_tool.as_deref().unwrap_or("none")
        );
        println!("  Input ready:    {}", req.all_input_ready);
        return Ok(());
    }

    // Detect backend
    let backend =
        detect_input_backend().map_err(|e| format!("Input backend not available: {e}"))?;
    println!("Backend: {} ({})", backend.kind, backend.binary_path);

    // Get display geometry for safety guard
    let (sw, sh) = match get_display_geometry(&backend).await {
        Ok(dims) => dims,
        Err(e) => {
            eprintln!("Warning: could not get display geometry: {e}");
            eprintln!("Using default 3440x1440");
            (3440, 1440)
        }
    };
    println!("Screen: {sw}x{sh}");

    let safety = InputSafetyGuard::new(sw, sh);
    let mouse = MouseController::new(backend.clone());
    let keyboard = KeyboardController::new(backend);

    let result: Result<String, String> = match command {
        "click" => {
            if args.len() < 4 {
                Err("Usage: nx-input click <x> <y> [button]".into())
            } else {
                let x = parse_u32(&args[2], "x")?;
                let y = parse_u32(&args[3], "y")?;
                let button = if args.len() > 4 {
                    parse_button(&args[4]).unwrap_or(MouseButton::Left)
                } else {
                    MouseButton::Left
                };
                let action = MouseAction::Click { x, y, button };
                safety
                    .validate_mouse_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = mouse.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Clicked {button} at ({x}, {y}) in {}ms [{}]",
                    r.duration_ms, r.audit_hash
                ))
            }
        }
        "dblclick" => {
            if args.len() < 4 {
                Err("Usage: nx-input dblclick <x> <y> [button]".into())
            } else {
                let x = parse_u32(&args[2], "x")?;
                let y = parse_u32(&args[3], "y")?;
                let button = if args.len() > 4 {
                    parse_button(&args[4]).unwrap_or(MouseButton::Left)
                } else {
                    MouseButton::Left
                };
                let action = MouseAction::DoubleClick { x, y, button };
                safety
                    .validate_mouse_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = mouse.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Double-clicked {button} at ({x}, {y}) in {}ms [{}]",
                    r.duration_ms, r.audit_hash
                ))
            }
        }
        "move" => {
            if args.len() < 4 {
                Err("Usage: nx-input move <x> <y>".into())
            } else {
                let x = parse_u32(&args[2], "x")?;
                let y = parse_u32(&args[3], "y")?;
                let action = MouseAction::Move { x, y };
                safety
                    .validate_mouse_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = mouse.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Moved to ({x}, {y}) in {}ms [{}]",
                    r.duration_ms, r.audit_hash
                ))
            }
        }
        "scroll" => {
            if args.len() < 6 {
                Err("Usage: nx-input scroll <x> <y> <direction> <amount>".into())
            } else {
                let x = parse_u32(&args[2], "x")?;
                let y = parse_u32(&args[3], "y")?;
                let direction = parse_scroll_dir(&args[4])
                    .ok_or_else(|| format!("Invalid direction: {}", args[4]))?;
                let amount = parse_u32(&args[5], "amount")?;
                let action = MouseAction::Scroll {
                    x,
                    y,
                    direction,
                    amount,
                };
                safety
                    .validate_mouse_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = mouse.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Scrolled {direction} {amount}x at ({x}, {y}) in {}ms [{}]",
                    r.duration_ms, r.audit_hash
                ))
            }
        }
        "drag" => {
            if args.len() < 6 {
                Err("Usage: nx-input drag <start_x> <start_y> <end_x> <end_y>".into())
            } else {
                let start_x = parse_u32(&args[2], "start_x")?;
                let start_y = parse_u32(&args[3], "start_y")?;
                let end_x = parse_u32(&args[4], "end_x")?;
                let end_y = parse_u32(&args[5], "end_y")?;
                let action = MouseAction::Drag {
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                };
                safety
                    .validate_mouse_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = mouse.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Dragged ({start_x},{start_y}) -> ({end_x},{end_y}) in {}ms [{}]",
                    r.duration_ms, r.audit_hash
                ))
            }
        }
        "position" => {
            let action = MouseAction::GetPosition;
            let r = mouse.execute(&action).await.map_err(|e| e.to_string())?;
            match r.position {
                Some((x, y)) => Ok(format!("Mouse position: ({x}, {y})")),
                None => Err("Could not determine mouse position".into()),
            }
        }
        "type" => {
            if args.len() < 3 {
                Err("Usage: nx-input type \"<text>\"".into())
            } else {
                let text = args[2..].join(" ");
                let action = KeyAction::Type { text: text.clone() };
                safety
                    .validate_keyboard_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = keyboard.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Typed {} chars in {}ms [{}]",
                    text.len(),
                    r.duration_ms,
                    r.audit_hash
                ))
            }
        }
        "key" => {
            if args.len() < 3 {
                Err("Usage: nx-input key <keyname>".into())
            } else {
                let action = KeyAction::KeyPress {
                    key: args[2].clone(),
                };
                safety
                    .validate_keyboard_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = keyboard.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Pressed {} in {}ms [{}]",
                    args[2], r.duration_ms, r.audit_hash
                ))
            }
        }
        "combo" => {
            if args.len() < 3 {
                Err("Usage: nx-input combo <key1+key2+...>".into())
            } else {
                let keys: Vec<String> = args[2].split('+').map(|s| s.to_string()).collect();
                let action = KeyAction::KeyCombo { keys };
                safety
                    .validate_keyboard_action(&action)
                    .map_err(|e| e.to_string())?;
                let r = keyboard.execute(&action).await.map_err(|e| e.to_string())?;
                Ok(format!(
                    "Combo {} in {}ms [{}]",
                    args[2], r.duration_ms, r.audit_hash
                ))
            }
        }
        other => Err(format!("Unknown command: {other}\n")),
    };

    match result {
        Ok(msg) => {
            println!("\n{msg}");
            Ok(())
        }
        Err(e) => {
            print_usage();
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(e) = run().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
