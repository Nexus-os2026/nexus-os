/// Test app governance for Nexus Computer Use
///
/// Usage:
///   nx-govern list          — list visible windows with categories
///   nx-govern focused       — show focused app info
///   nx-govern grants        — show active grants
///   nx-govern grant <class> <level> — manually grant access
///   nx-govern test-click    — try clicking, show governance check
use nexus_computer_use::governance::app_grant::{AppGrant, AppGrantManager, GrantLevel};
use nexus_computer_use::governance::app_registry::AppRegistry;
use nexus_computer_use::governance::session::GovernedSession;

fn print_usage() {
    eprintln!("Usage: nx-govern <COMMAND>");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  list              List visible windows with categories");
    eprintln!("  focused           Show focused app info and its grant level");
    eprintln!("  grants            Show all active grants");
    eprintln!("  grant <wm> <lvl>  Manually grant access (levels: full, click, readonly)");
    eprintln!("  test-click        Try a click action, show governance result");
    eprintln!("  test-type         Try a type action, show governance result");
    eprintln!("  session           Show session info and audit summary");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  nx-govern list");
    eprintln!("  nx-govern focused");
    eprintln!("  nx-govern grant discord full");
    eprintln!("  nx-govern test-click");
}

fn parse_grant_level(s: &str) -> Option<GrantLevel> {
    match s.to_lowercase().as_str() {
        "full" => Some(GrantLevel::Full),
        "click" => Some(GrantLevel::Click),
        "readonly" | "read" => Some(GrantLevel::ReadOnly),
        "restricted" => Some(GrantLevel::Restricted),
        _ => None,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let command = args[1].as_str();

    match command {
        "list" => cmd_list().await,
        "focused" => cmd_focused().await,
        "grants" => cmd_grants(),
        "grant" => {
            if args.len() < 4 {
                eprintln!("Usage: nx-govern grant <wm_class> <level>");
                eprintln!("Levels: full, click, readonly, restricted");
                std::process::exit(1);
            }
            cmd_grant(&args[2], &args[3]).await;
        }
        "test-click" => cmd_test_action("click").await,
        "test-type" => cmd_test_action("type").await,
        "session" => cmd_session().await,
        _ => {
            eprintln!("Unknown command: {command}");
            print_usage();
            std::process::exit(1);
        }
    }
}

async fn cmd_list() {
    let registry = AppRegistry::new();
    match registry.list_visible_apps().await {
        Ok(apps) => {
            println!("Visible windows ({} found):", apps.len());
            println!(
                "{:<20} {:<25} {:<15} {:<8} {:<10}",
                "Name", "WM_CLASS", "Category", "PID", "Focused"
            );
            println!("{}", "-".repeat(80));
            for app in &apps {
                println!(
                    "{:<20} {:<25} {:<15} {:<8} {:<10}",
                    truncate(&app.name, 19),
                    truncate(&app.wm_class, 24),
                    app.category.to_string(),
                    app.pid,
                    if app.is_focused { "YES" } else { "" }
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to list windows: {e}");
            eprintln!("Make sure xdotool is installed and X11 is running.");
            std::process::exit(1);
        }
    }
}

async fn cmd_focused() {
    let registry = AppRegistry::new();
    let manager = AppGrantManager::new();

    match registry.get_focused_app().await {
        Ok(app) => {
            println!("Focused Application:");
            println!("  Name:      {}", app.name);
            println!("  WM_CLASS:  {}", app.wm_class);
            println!("  PID:       {}", app.pid);
            println!("  Window ID: 0x{:08x}", app.window_id);
            println!("  Title:     {}", app.title);
            println!("  Category:  {}", app.category);
            println!("  Grant:     {}", manager.effective_level(&app));
        }
        Err(e) => {
            eprintln!("Failed to get focused app: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_grants() {
    let manager = AppGrantManager::new();
    let grants = manager.active_grants();

    if grants.is_empty() {
        println!("No explicit grants. Default auto-grants are active:");
        println!("  Terminal     → Full");
        println!("  Editor       → Full");
        println!("  NexusOS      → Full");
        println!("  Browser      → Click");
        println!("  FileManager  → Click");
        println!("  Communication → ReadOnly");
        println!("  System       → ReadOnly");
        println!("  Unknown      → ReadOnly");
    } else {
        println!("Active grants ({}):", grants.len());
        for grant in &grants {
            println!(
                "  [{}] {} → {} (by: {}, hash: {}..)",
                grant.id,
                grant.app_wm_class,
                grant.grant_level,
                grant.granted_by,
                &grant.audit_hash[..16],
            );
        }
    }
}

async fn cmd_grant(wm_class: &str, level_str: &str) {
    let registry = AppRegistry::new();
    let level = match parse_grant_level(level_str) {
        Some(l) => l,
        None => {
            eprintln!("Invalid grant level: {level_str}");
            eprintln!("Valid levels: full, click, readonly, restricted");
            std::process::exit(1);
        }
    };

    let category = registry.categorize(wm_class);
    let grant = AppGrant::new(wm_class, category, level.clone(), Vec::new(), "user", None);

    println!("Grant created:");
    println!("  ID:        {}", grant.id);
    println!("  App:       {wm_class}");
    println!("  Level:     {level}");
    println!("  Hash:      {}", grant.audit_hash);
    println!("  Granted:   {}", grant.granted_at);
    println!();
    println!("Note: This grant is for demonstration only.");
    println!("In a real session, grants persist for the session lifetime.");
}

async fn cmd_test_action(action_type: &str) {
    let registry = AppRegistry::new();
    let mut session = GovernedSession::with_defaults();

    let focused = match registry.get_focused_app().await {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Failed to get focused app: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "Focused: {} ({}) [{}]",
        focused.name, focused.wm_class, focused.category
    );
    println!(
        "Grant level: {}",
        session.grant_manager.effective_level(&focused)
    );
    println!();

    let action = match action_type {
        "click" => nexus_computer_use::agent::action::AgentAction::Click {
            x: 100,
            y: 100,
            button: "left".to_string(),
        },
        "type" => nexus_computer_use::agent::action::AgentAction::Type {
            text: "test".to_string(),
        },
        _ => {
            eprintln!("Unknown action type");
            std::process::exit(1);
        }
    };

    println!("Testing action: {action}");
    match session.validate_action(&focused, &action) {
        Ok(grant_id) => {
            println!("  ALLOWED (grant: {grant_id})");
        }
        Err(e) => {
            println!("  DENIED: {e}");
        }
    }
}

async fn cmd_session() {
    let session = GovernedSession::with_defaults();
    println!("Session Info:");
    println!("  ID:       {}", session.id);
    println!("  Started:  {}", session.started_at);
    println!("  Timeout:  {}min", session.config.session_timeout_minutes);
    println!("  Max acts: {}", session.config.max_actions_per_app);
    println!("  Actions:  {}", session.total_actions());
    println!();
    println!("Audit: {}", session.audit_summary());
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
