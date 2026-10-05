//! The agent display server (Linux): a backend-owned Xvfb.
//!
//! It runs on a display number Nexus picks (never the owner's), listens on
//! no network and no abstract socket, admits only clients that present the
//! private MIT-MAGIC-COOKIE written for it, and dies with Nexus. Nexus talks
//! to it over the filesystem socket with that cookie, explicitly: nothing
//! here reads `DISPLAY` or `XAUTHORITY`. It connects only to a socket that
//! this user owns in a socket directory no other user can replace entries
//! of, and only while its own server runs, so another local user cannot
//! stand in for the agent display. The server is stopped with `SIGTERM`
//! first, so it removes its lock file and socket.

use super::{Rect, WindowInfo};
use crate::authority::AuthorityError;
use crate::executable::{inspect, Trust};
use crate::launcher::{SessionProcess, SessionSpec};
use crate::runtime_root::{RuntimeRoot, Scratch};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, AtomEnum, ConnectionExt as _, ImageFormat, MapState, Window};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::{DefaultStream, RustConnection};

/// Where X servers put their sockets.
const SOCKET_DIR: &str = "/tmp/.X11-unix/X";
/// How long the server may take to remove its lock and socket when stopped.
const STOP_GRACE: Duration = Duration::from_secs(1);
/// The X input focus value meaning "the window under the pointer".
const POINTER_ROOT: u32 = 1;
const XVFB: &str = "/usr/bin/Xvfb";
const COOKIE: &[u8] = b"MIT-MAGIC-COOKIE-1";

/// A running agent display.
pub(crate) struct AgentServer {
    // Declared first: the connection closes before the server ends.
    conn: Mutex<RustConnection>,
    root: Window,
    width: u16,
    height: u16,
    number: u32,
    #[cfg(test)]
    cookie: [u8; 16],
    _process: SessionProcess,
    _scratch: Scratch,
    keymap: Keymap,
}

struct Keymap {
    min: u8,
    per: usize,
    syms: Vec<u32>,
}

/// The server held by Nexus (no other client is served) until this drops.
pub(crate) struct Hold<'a>(&'a AgentServer);

impl Drop for Hold<'_> {
    fn drop(&mut self) {
        let conn = self.0.conn.lock().unwrap_or_else(|p| p.into_inner());
        let _ = conn.ungrab_server();
        let _ = AgentServer::sync(&conn);
    }
}

fn unavailable(what: &'static str) -> AuthorityError {
    AuthorityError::Unavailable(what)
}

/// The authorization file the server reads: one MIT-MAGIC-COOKIE-1 entry.
fn authority_entry(number: u32, cookie: &[u8; 16]) -> Vec<u8> {
    fn field(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(bytes);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0100u16.to_be_bytes()); // FamilyLocal
    field(&mut out, b"nexus-agent-display");
    field(&mut out, number.to_string().as_bytes());
    field(&mut out, COOKIE);
    field(&mut out, cookie);
    out
}

fn random_bytes<const N: usize>() -> Result<[u8; N], AuthorityError> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).map_err(|_| unavailable("no randomness"))?;
    Ok(bytes)
}

fn socket(number: u32) -> PathBuf {
    PathBuf::from(format!("{SOCKET_DIR}{number}"))
}

fn socket_is_ours(number: u32, uid: u32) -> bool {
    socket_trusted(Path::new("/tmp/.X11-unix"), &socket(number), uid)
}

/// Whether `entry` in `dir` can only be this user's (`uid`): `dir` is a real
/// directory owned by root or this user, and either sticky or writable by no
/// one else (so no other user can remove or replace an entry they do not
/// own), and `entry` is a socket (not a link) owned by this user.
pub(super) fn socket_trusted(dir: &Path, entry: &Path, uid: u32) -> bool {
    let trusted_dir = std::fs::symlink_metadata(dir).is_ok_and(|dir| {
        dir.is_dir()
            && (dir.uid() == 0 || dir.uid() == uid)
            && (dir.mode() & 0o1000 != 0 || dir.mode() & 0o022 == 0)
    });
    trusted_dir
        && std::fs::symlink_metadata(entry)
            .is_ok_and(|entry| entry.file_type().is_socket() && entry.uid() == uid)
}

fn connect(number: u32, cookie: &[u8; 16], uid: u32) -> Option<RustConnection> {
    if !socket_is_ours(number, uid) {
        return None;
    }
    let stream = UnixStream::connect(socket(number)).ok()?;
    let (stream, _) = DefaultStream::from_unix_stream(stream).ok()?;
    RustConnection::connect_to_stream_with_auth_info(stream, 0, COOKIE.to_vec(), cookie.to_vec())
        .ok()
}

impl AgentServer {
    /// Start an agent display of `width` x `height`.
    pub(crate) fn start(
        root: &RuntimeRoot,
        width: u16,
        height: u16,
    ) -> Result<Self, AuthorityError> {
        let program = inspect(Path::new(XVFB), Trust::System)?;
        for _ in 0..8 {
            let [a, b] = random_bytes::<2>()?;
            let number = 200 + (u32::from(u16::from_be_bytes([a, b])) % 800);
            if socket(number).exists() {
                continue;
            }
            let cookie = random_bytes::<16>()?;
            let scratch = root.scratch("display")?;
            // This user's id: the owner of the directory just created.
            let uid = std::fs::metadata(scratch.path())
                .map_err(|_| unavailable("no display directory"))?
                .uid();
            let auth = scratch.write_new("authority", &authority_entry(number, &cookie))?;
            let process = SessionProcess::launch(SessionSpec {
                program: program.path.clone(),
                args: vec![
                    format!(":{number}").into(),
                    "-screen".into(),
                    "0".into(),
                    format!("{width}x{height}x24").into(),
                    "-nolisten".into(),
                    "tcp".into(),
                    "-nolisten".into(),
                    "local".into(),
                    "-auth".into(),
                    auth.into_os_string(),
                    "-noreset".into(),
                ],
                env: vec![],
                current_dir: scratch.path().to_path_buf(),
                stop_grace: Some(STOP_GRACE),
                inherit: vec![],
            })?;
            let mut process = process;
            let deadline = Instant::now() + Duration::from_secs(10);
            let conn = loop {
                // Only while our own server runs: a server that failed to
                // start leaves the number to whoever holds it.
                if let Some(conn) = connect(number, &cookie, uid).filter(|_| process.running()) {
                    break Some(conn);
                }
                if !process.running() || Instant::now() >= deadline {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            };
            let Some(conn) = conn else {
                process.end();
                continue;
            };
            let screen = &conn.setup().roots[0];
            let (root_window, w, h) =
                (screen.root, screen.width_in_pixels, screen.height_in_pixels);
            let keymap = Self::keymap(&conn)?;
            return Ok(Self {
                conn: Mutex::new(conn),
                root: root_window,
                width: w,
                height: h,
                number,
                #[cfg(test)]
                cookie,
                _process: process,
                _scratch: scratch,
                keymap,
            });
        }
        Err(unavailable("the agent display could not start"))
    }

    fn keymap(conn: &RustConnection) -> Result<Keymap, AuthorityError> {
        let setup = conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let reply = conn
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(|_| unavailable("the agent display does not answer"))?
            .reply()
            .map_err(|_| unavailable("the agent display does not answer"))?;
        Ok(Keymap {
            min,
            per: usize::from(reply.keysyms_per_keycode),
            syms: reply.keysyms,
        })
    }

    pub(crate) fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub(crate) fn number(&self) -> u32 {
        self.number
    }

    /// A connection for a test client of this display (tests only).
    #[cfg(test)]
    pub(crate) fn client(&self) -> RustConnection {
        let uid = std::fs::metadata(self._scratch.path())
            .expect("the display directory")
            .uid();
        connect(self.number, &self.cookie, uid).expect("a test client connects")
    }

    /// Whether the server still answers.
    pub(crate) fn alive(&self) -> bool {
        let conn = self.conn.lock().expect("display");
        conn.get_input_focus()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some()
    }

    /// The pixels of `rect` as packed RGB.
    pub(crate) fn capture(&self, rect: Rect) -> Result<Vec<u8>, AuthorityError> {
        let conn = self.conn.lock().expect("display");
        let reply = conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                rect.x as i16,
                rect.y as i16,
                rect.width,
                rect.height,
                !0,
            )
            .map_err(|_| unavailable("the agent display does not answer"))?
            .reply()
            .map_err(|_| unavailable("the agent display could not be captured"))?;
        let pixels = usize::from(rect.width) * usize::from(rect.height);
        if reply.depth != 24 || reply.data.len() != pixels * 4 {
            return Err(unavailable("the agent display has an unexpected format"));
        }
        let mut rgb = Vec::with_capacity(pixels * 3);
        for pixel in reply.data.chunks_exact(4) {
            // Z-pixmap, 32 bits per pixel, least significant byte first.
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        Ok(rgb)
    }

    fn title(conn: &RustConnection, window: Window) -> String {
        let utf8 = conn
            .intern_atom(true, b"UTF8_STRING")
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom);
        let net_name = conn
            .intern_atom(true, b"_NET_WM_NAME")
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom);
        if let (Some(utf8), Some(net_name)) = (utf8, net_name) {
            if net_name != 0 {
                if let Some(reply) = conn
                    .get_property(false, window, net_name, utf8, 0, 256)
                    .ok()
                    .and_then(|c| c.reply().ok())
                {
                    if !reply.value.is_empty() {
                        return String::from_utf8_lossy(&reply.value).into_owned();
                    }
                }
            }
        }
        conn.get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 256)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|reply| reply.value.iter().map(|&b| char::from(b)).collect())
            .unwrap_or_default()
    }

    /// Viewable top-level windows, bottom to top.
    pub(crate) fn windows(&self) -> Vec<WindowInfo> {
        let conn = self.conn.lock().expect("display");
        let Some(tree) = conn.query_tree(self.root).ok().and_then(|c| c.reply().ok()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for window in tree.children {
            let viewable = conn
                .get_window_attributes(window)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|a| a.map_state == MapState::VIEWABLE);
            if !viewable {
                continue;
            }
            let Some(geometry) = conn.get_geometry(window).ok().and_then(|c| c.reply().ok()) else {
                continue;
            };
            out.push(WindowInfo {
                id: window,
                title: Self::title(&conn, window),
                rect: Rect {
                    x: geometry.x.max(0) as u16,
                    y: geometry.y.max(0) as u16,
                    width: geometry.width,
                    height: geometry.height,
                },
            });
        }
        out
    }

    /// Where the pointer is.
    pub(crate) fn pointer_position(&self) -> (u16, u16) {
        let conn = self.conn.lock().expect("display");
        conn.query_pointer(self.root)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| (r.root_x.max(0) as u16, r.root_y.max(0) as u16))
            .unwrap_or((0, 0))
    }

    /// The top-most viewable window containing the point, if any.
    pub(crate) fn window_at(&self, x: u16, y: u16) -> Option<WindowInfo> {
        self.windows()
            .into_iter()
            .rev()
            .find(|w| w.rect.contains(x, y))
    }

    /// Hold the server: until the guard drops, the X server serves no
    /// other client, so what Nexus checks stays true for the event it sends
    /// next.
    pub(crate) fn hold(&self) -> Result<Hold<'_>, AuthorityError> {
        let conn = self.conn.lock().expect("display");
        conn.grab_server()
            .map_err(|_| unavailable("the agent display does not answer"))?;
        Self::sync(&conn)?;
        Ok(Hold(self))
    }

    /// Whether keyboard events now reach `window` (0: the display
    /// background): the focus follows the pointer (which the caller has
    /// checked is over `window`), or it is `window` or a window inside it.
    pub(crate) fn keys_reach(&self, window: u32) -> bool {
        let conn = self.conn.lock().expect("display");
        let Some(focus) = conn
            .get_input_focus()
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.focus)
        else {
            return false;
        };
        if focus == POINTER_ROOT {
            return true;
        }
        let target = if window == 0 { self.root } else { window };
        let mut current = focus;
        for _ in 0..64 {
            if current == target {
                return true;
            }
            if current == x11rb::NONE || current == self.root {
                return false;
            }
            match conn.query_tree(current).ok().and_then(|c| c.reply().ok()) {
                Some(tree) => current = tree.parent,
                None => return false,
            }
        }
        false
    }

    fn sync(conn: &RustConnection) -> Result<(), AuthorityError> {
        conn.get_input_focus()
            .map_err(|_| unavailable("the agent display does not answer"))?
            .reply()
            .map(|_| ())
            .map_err(|_| unavailable("the agent display does not answer"))
    }

    fn fake(&self, kind: u8, detail: u8, x: i16, y: i16) -> Result<(), AuthorityError> {
        let conn = self.conn.lock().expect("display");
        conn.xtest_fake_input(kind, detail, 0, self.root, x, y, 0)
            .map_err(|_| unavailable("the agent display refused input"))?;
        Self::sync(&conn)
    }

    pub(crate) fn pointer(&self, x: u16, y: u16) -> Result<(), AuthorityError> {
        self.fake(xproto::MOTION_NOTIFY_EVENT, 0, x as i16, y as i16)
    }

    pub(crate) fn button(&self, button: u8, press: bool) -> Result<(), AuthorityError> {
        let kind = if press {
            xproto::BUTTON_PRESS_EVENT
        } else {
            xproto::BUTTON_RELEASE_EVENT
        };
        self.fake(kind, button, 0, 0)
    }

    pub(crate) fn key(&self, keycode: u8, press: bool) -> Result<(), AuthorityError> {
        let kind = if press {
            xproto::KEY_PRESS_EVENT
        } else {
            xproto::KEY_RELEASE_EVENT
        };
        self.fake(kind, keycode, 0, 0)
    }

    /// The keycode producing `keysym`, and whether Shift is needed.
    pub(crate) fn keycode(&self, keysym: u32) -> Option<(u8, bool)> {
        let map = &self.keymap;
        if map.per == 0 {
            return None;
        }
        for (index, chunk) in map.syms.chunks(map.per).enumerate() {
            let code = map.min.checked_add(u8::try_from(index).ok()?)?;
            if chunk.first() == Some(&keysym) {
                return Some((code, false));
            }
            if chunk.get(1) == Some(&keysym) {
                return Some((code, true));
            }
        }
        None
    }
}
