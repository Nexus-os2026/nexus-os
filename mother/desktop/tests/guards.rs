//! M2b-A PR-2 source guards over the desktop crate (design §7.1 S3, S9,
//! G9, G10, G12, G13, G15), registered before the code they judge. Each
//! scanner first proves, on planted text, that it flags what it must.

use nexus_mother_desktop::inspector::inspector_requested;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The desktop's production sources: `src/`, as `(file name, text)`, with
/// comment lines removed.
fn production() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(crate_dir().join("src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            out.push((name, code(&std::fs::read_to_string(&path).unwrap())));
        }
    }
    out.sort();
    assert!(
        out.len() >= 7,
        "{:?}",
        out.iter().map(|f| &f.0).collect::<Vec<_>>()
    );
    out
}

fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn source(name: &str) -> String {
    production()
        .into_iter()
        .find(|(file, _)| file == name)
        .unwrap_or_else(|| panic!("src/{name}"))
        .1
}

/// The body of `fn name(` (or `fn name<`) in `text`, braces matched.
fn body_of<'a>(text: &'a str, name: &str) -> &'a str {
    let at = text
        .find(&format!("fn {name}("))
        .or_else(|| text.find(&format!("fn {name}<")))
        .unwrap_or_else(|| panic!("fn {name}"));
    let open = at + text[at..].find('{').unwrap();
    let mut depth = 0;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open + 1..open + i];
                }
            }
            _ => {}
        }
    }
    panic!("fn {name}: unbalanced");
}

fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

// ── S3: the start order ─────────────────────────────────────────────────────

/// In `launch`, the inspector check comes first, then the broker is opened,
/// then the main context is owned, then `gtk::init`.
fn start_order_holds(launch: &str) -> Result<(), String> {
    let at = |needle: &str| {
        launch
            .find(needle)
            .ok_or_else(|| format!("launch does not call {needle}"))
    };
    let inspector = at("inspector_requested(")?;
    let broker = at("open(home_dir)")?;
    let context = at(".acquire()")?;
    let init = at("gtk::init()")?;
    if inspector < broker && broker < context && context < init {
        Ok(())
    } else {
        Err(format!(
            "order: inspector {inspector}, broker {broker}, context {context}, init {init}"
        ))
    }
}

#[test]
fn s3_the_scanner_rejects_gtk_before_the_home() {
    let planted = "fn launch() { if inspector_requested(d) {} let c = ctx.acquire(); gtk::init()?; let o = open(home_dir)?; }";
    assert!(start_order_holds(planted).is_err());
    let planted = "fn launch() { if inspector_requested(d) {} let o = open(home_dir)?; let c = ctx.acquire(); gtk::init()?; }";
    assert!(start_order_holds(planted).is_ok());
}

#[test]
fn s3_home_is_opened_before_gtk_init() {
    let app = source("app.rs");
    start_order_holds(body_of(&app, "launch")).unwrap();
    // The production start opens the broker through `Home::open`, with the
    // desktop's confirmer, which answers no; and `main` goes through it.
    let start = squash(body_of(&app, "start"));
    assert!(
        start.contains("Home::open(dir,Arc::new(NoDialogs))"),
        "{start}"
    );
    assert!(start.contains("launch(home_dir,gtk_debug,"), "{start}");
    let entry = squash(body_of(&app, "main_entry"));
    assert!(entry.contains("start(&home,"), "{entry}");
    assert_eq!(app.matches("gtk::init()").count(), 1);
    assert!(squash(&source("main.rs")).contains("app::main_entry()"));
}

// ── S9: the inspector request is recognised ─────────────────────────────────

#[test]
fn s9_inspector_requests_are_recognised() {
    for value in [
        "interactive",
        "INTERACTIVE",
        "all",
        "actions,interactive",
        "keybindings:interactive",
        " interactive ",
        "size-request,all",
    ] {
        assert!(inspector_requested(Some(OsStr::new(value))), "{value:?}");
    }
    for value in ["", "actions", "keybindings,size-request", "interactiveness"] {
        assert!(!inspector_requested(Some(OsStr::new(value))), "{value:?}");
    }
    assert!(!inspector_requested(None));
    use std::os::unix::ffi::OsStrExt;
    assert!(
        inspector_requested(Some(OsStr::from_bytes(b"\xff"))),
        "unreadable text counts as asking"
    );
}

// ── G9: the dependency allow-list ───────────────────────────────────────────

/// The names in one `[section]` of the manifest.
fn section(manifest: &str, header: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == header;
            continue;
        }
        if inside && !line.is_empty() && !line.starts_with('#') {
            names.push(line.split(['=', ' ']).next().unwrap().to_string());
        }
    }
    names.sort();
    names
}

#[test]
fn g9_the_desktop_depends_only_on_its_allow_list() {
    let manifest = std::fs::read_to_string(crate_dir().join("Cargo.toml")).unwrap();
    assert_eq!(
        section(&manifest, "[dependencies]"),
        [
            "gtk4",
            "nexus-governed-control",
            "nexus-kernel",
            "nexus-mother-host"
        ]
    );
    assert!(
        manifest.contains("gtk4 = \"=0.11.5\""),
        "gtk4 is pinned exactly"
    );
    // Decision 3: one test-only dependency, pinned: the kernel's nix, for the
    // live harness's native process-group signalling.
    assert_eq!(section(&manifest, "[dev-dependencies]"), ["nix"]);
    assert!(
        manifest.contains(
            "nix = { version = \"0.29\", default-features = false, features = [\"process\", \"signal\"] }"
        ),
        "nix is pinned to the kernel's 0.29 with process and signal only"
    );
    assert_eq!(
        section(&manifest, "[build-dependencies]"),
        Vec::<String>::new()
    );
    assert!(!manifest.contains("[target."), "no target tables");
    assert!(!manifest.contains("[features]"), "no features");
    assert!(!Path::new(&crate_dir().join("build.rs")).exists());
}

// ── G10: the one confirmer answers no; the broker is reached only as built ──

const DIALOGS: [&str; 7] = [
    "confirm_action",
    "confirm_grant",
    "confirm_resume",
    "confirm_acknowledgement",
    "confirm_mission",
    "confirm_agent_door",
    "confirm_held_mission",
];

#[test]
fn g10_the_one_confirmer_answers_no_to_every_question() {
    let marker = ["ControlConfirmer", " for "].concat();
    let impls: Vec<String> = production()
        .iter()
        .filter(|(_, text)| text.contains(&marker))
        .map(|(file, text)| format!("{file}:{}", text.matches(&marker).count()))
        .collect();
    assert_eq!(impls, ["confirm.rs:1"]);
    let confirm = source("confirm.rs");
    for dialog in DIALOGS {
        assert_eq!(squash(body_of(&confirm, dialog)), "false", "{dialog}");
    }
    assert!(!confirm.contains("stdin"));
}

/// Calls the desktop's production code never makes: no broker or store
/// construction of its own, no Stop controller, nothing that raises
/// authority. The Stop route calls only `emergency_stop`.
const NEVER: [&str; 17] = [
    "GovernedControl::new",
    "PrimaryEvidence::open",
    // Built from parts: the kernel's Stop-seal scan allows no other file
    // to name the process's controller claim.
    concat!("claim_global", "_controller"),
    "claim_controller",
    "lift_if",
    ".engage(",
    ".resume(",
    ".authorize(",
    ".execute(",
    ".deny(",
    "request_grant",
    "revoke_grant",
    "issue_agent_door",
    "request_consent",
    ".door(",
    "Connection::open",
    "std::io::stdin",
];

#[test]
fn g10_no_authority_call_in_the_desktop() {
    for (file, text) in production() {
        for needle in NEVER {
            assert!(!text.contains(needle), "src/{file}: {needle}");
        }
    }
    let stop = source("stop.rs");
    assert_eq!(stop.matches(".emergency_stop()").count(), 1, "stop.rs");
    assert_eq!(stop.matches(".shut_down()").count(), 1, "stop.rs");
    for (file, text) in production() {
        if file != "stop.rs" {
            assert!(!text.contains(".emergency_stop()"), "src/{file}");
            assert!(!text.contains(".shut_down()"), "src/{file}");
        }
    }
}

// ── G12: text only, never markup ────────────────────────────────────────────

const MARKUP: [&str; 5] = [
    "set_markup",
    "use_markup",
    "markup_escape",
    "with_markup",
    "set_use_markup",
];

#[test]
fn g12_no_markup_api_in_the_desktop() {
    for (file, text) in production() {
        for needle in MARKUP {
            assert!(!text.contains(needle), "src/{file}: {needle}");
        }
    }
}

// ── G13 (scan): no application, no bus, no export, no app accelerators ─────

const BUS: [&str; 12] = [
    "gtk::Application",
    "ApplicationWindow",
    "gio::Application",
    "Application::new",
    "Application::builder",
    "DBusConnection",
    "export_action_group",
    "export_menu_model",
    "bus_own_name",
    "bus_get",
    "set_accels_for_action",
    "set_application",
];

#[test]
fn g13_no_application_and_no_bus_export_in_the_desktop() {
    for (file, text) in production() {
        for needle in BUS {
            assert!(!text.contains(needle), "src/{file}: {needle}");
        }
    }
}

// ── G15: every GTK callback body goes through `guarded` ────────────────────

/// Where a callback is registered: each must take a closure whose body
/// starts with `guarded(`.
const REGISTRATIONS: [&str; 6] = [
    ".connect_",
    "timeout_add_local",
    "timeout_add",
    "idle_add_local",
    "idle_add",
    "CallbackAction::new(",
];

/// Each callback registration in `text` whose closure body does not start
/// with `guarded(`: `(offset, the registration's text)`.
fn unguarded(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let next = REGISTRATIONS
            .iter()
            .filter_map(|r| text[at..].find(r).map(|i| (at + i, *r)))
            .min();
        let Some((start, registration)) = next else {
            break;
        };
        at = start + registration.len();
        if text[..start].ends_with("fn ") {
            continue;
        }
        // The closure: the first `|` after the registration, then its body.
        let rest = &text[at..];
        let Some(bar) = rest.find('|') else {
            found.push(format!("{start}: {registration} without a closure"));
            continue;
        };
        let after_params = rest[bar + 1..].find('|').map(|i| bar + 1 + i + 1);
        let Some(body) = after_params else {
            found.push(format!("{start}: {registration} without a closure"));
            continue;
        };
        let tail = rest[body..].trim_start();
        let tail = tail.strip_prefix('{').map(str::trim_start).unwrap_or(tail);
        if !tail.starts_with("guarded(") {
            let end = (start + 60).min(text.len());
            found.push(format!("{start}: {}", text[start..end].replace('\n', " ")));
        }
    }
    found
}

#[test]
fn g15_the_scanner_flags_an_unguarded_callback() {
    let planted = "button.connect_clicked(move |_| { route.press(); });";
    assert_eq!(unguarded(planted).len(), 1);
    let planted = "glib::timeout_add_local(every, move || drain());";
    assert_eq!(unguarded(planted).len(), 1);
    let planted = "CallbackAction::new(|_, _| { stop() })";
    assert_eq!(unguarded(planted).len(), 1);
    let fine = "button.connect_clicked(move |_| {\n    guarded(\"stop\", || route.press());\n});\nglib::timeout_add_local(every, move || guarded(\"drain\", || drain()).unwrap_or(glib::ControlFlow::Break));";
    assert_eq!(unguarded(fine), Vec::<String>::new());
}

#[test]
fn g15_every_gtk_callback_in_the_desktop_goes_through_guarded() {
    let mut registrations = 0;
    for (file, text) in production() {
        registrations += REGISTRATIONS
            .iter()
            .map(|r| text.matches(r).count())
            .sum::<usize>();
        assert_eq!(unguarded(&text), Vec::<String>::new(), "src/{file}");
    }
    assert!(registrations >= 4, "the window registers its callbacks");
    // `guarded` contains the panic itself.
    let window = source("window.rs");
    assert!(body_of(&window, "guarded").contains("catch_unwind("));
}
