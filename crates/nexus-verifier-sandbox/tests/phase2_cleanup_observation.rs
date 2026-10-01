//! P2-V1-R1 to R3A fixture controls for the live harness's entry and its
//! checked cleanup observations (`support/cleanup_observation.rs`): the
//! observation is one bounded call in this process, never a subprocess; a
//! failed, refused, timed-out, oversized or malformed observation is never "no
//! scopes"; a workspace inspection that cannot answer is never "none"; both
//! observations always run; the arguments alone choose the entry, and only no
//! argument enters the suite; a retained boundary's owner survives every
//! failed cleanup attempt, and no failure hides another.
//!
//! These are not live evidence. No user manager or bus daemon is contacted,
//! no process is started by an observation, and no unit, scope or cgroup is
//! created. The bus is a fixture peer this test owns at the other end of a
//! socket pair (or of a listening socket in a fixture runtime directory),
//! served by one thread that is always joined. The peer scripts the bus side
//! of the conversation (the EXTERNAL handshake's replies, Hello's answer and
//! the call's answer) with zbus's own message parser and builder, and records
//! every message the observer sent, so the request itself is checked.
//! Runtime directories are fixture trees under the temporary directory (in
//! which this test's uid stands in for root); failures this host's
//! permissions cannot produce deterministically are injected. The entry is
//! proved with stand-in entries only: the real suite never runs here.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/cleanup_observation.rs"]
mod cleanup_observation;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod controls {
    use super::cleanup_observation::*;
    use std::cell::{Cell, RefCell};
    use std::ffi::{CString, OsString};
    use std::fs;
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use zbus::message::Type;
    use zbus::zvariant::serialized::{Context, Data};
    use zbus::zvariant::{DynamicType, Endian, OwnedObjectPath};
    use zbus::Message;

    const AMBIENT_CHILD: &str = "NEXUS_P2V1R3A_AMBIENT_CHILD";
    /// Bound on every wait of a fixture peer: it never hangs a test.
    const PEER_TIMEOUT: Duration = Duration::from_secs(20);
    const GUID: &str = "0123456789abcdef0123456789abcdef";

    fn uid() -> u32 {
        // SAFETY: getuid has no preconditions.
        unsafe { libc::getuid() }
    }

    fn euid() -> u32 {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() }
    }

    fn mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// A fixture root this test owns, removed when dropped.
    struct Root(PathBuf);

    impl Root {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "p2v1r3a-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir(&root).unwrap();
            mode(&root, 0o755);
            Root(root)
        }

        /// `run/user/<uid>` as a host has it: `run` and `run/user` 0755, the
        /// runtime directory 0700 holding a listening bus socket.
        fn runtime(&self, uid: u32) -> (PathBuf, UnixListener) {
            let user = self.0.join("run/user");
            fs::create_dir_all(&user).unwrap();
            mode(&self.0.join("run"), 0o755);
            mode(&user, 0o755);
            let runtime = user.join(uid.to_string());
            fs::create_dir(&runtime).unwrap();
            mode(&runtime, 0o700);
            let bus = UnixListener::bind(runtime.join("bus")).unwrap();
            (runtime, bus)
        }

        /// This root as a host whose root is this test's uid.
        fn host(&self, uid: u32, fs_magic: Option<i64>) -> Host<'_> {
            Host {
                root: &self.0,
                uid,
                root_owner: self::uid(),
                fs_magic,
            }
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The peer's answer to the listing call, built from the call itself.
    type Reply = Box<dyn FnOnce(&Message) -> Message + Send>;

    fn answer_rows(rows: Vec<UnitRow>) -> Option<Reply> {
        Some(Box::new(move |call: &Message| {
            Message::method_reply(call).unwrap().build(&rows).unwrap()
        }))
    }

    fn answer_body<B>(body: B) -> Option<Reply>
    where
        B: serde::Serialize + DynamicType + Send + 'static,
    {
        Some(Box::new(move |call: &Message| {
            Message::method_reply(call).unwrap().build(&body).unwrap()
        }))
    }

    fn answer_error(name: &'static str, text: String) -> Option<Reply> {
        Some(Box::new(move |call: &Message| {
            Message::method_error(call, name)
                .unwrap()
                .build(&text)
                .unwrap()
        }))
    }

    /// How the fixture peer conducts the bus side of the handshake.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Handshake {
        /// Accept EXTERNAL and answer Hello.
        Accept,
        /// Refuse every authentication.
        Reject,
        /// Never answer the authentication.
        Silent,
        /// Answer Hello with an error.
        FailHello,
        /// Close the connection once Hello is answered.
        CloseAfterHello,
    }

    /// A message the observer sent, as the peer saw it.
    #[derive(Debug, PartialEq)]
    struct Sent {
        kind: Type,
        destination: Option<String>,
        path: Option<String>,
        interface: Option<String>,
        member: Option<String>,
        signature: Option<String>,
        /// The arguments of an `asas` body.
        arguments: Option<(Vec<String>, Vec<String>)>,
        fds: usize,
    }

    impl Sent {
        fn of(message: &Message) -> Self {
            let header = message.header();
            let body = message.body();
            let signature = body.signature().map(|s| s.as_str().to_string());
            let arguments = (signature.as_deref() == Some("asas"))
                .then(|| body.deserialize::<(Vec<String>, Vec<String>)>().ok())
                .flatten();
            Sent {
                kind: message.message_type(),
                destination: header.destination().map(|name| name.to_string()),
                path: header.path().map(|path| path.to_string()),
                interface: header.interface().map(|name| name.to_string()),
                member: header.member().map(|name| name.to_string()),
                signature,
                arguments,
                fds: message.data().fds().len(),
            }
        }
    }

    /// What the fixture peer saw of one observation.
    #[derive(Debug, Default)]
    struct Seen {
        /// The authentication command, its leading NUL aside.
        auth: String,
        /// Every message the observer sent, Hello first.
        messages: Vec<Sent>,
        /// The observer closed its end: the end of the conversation, read.
        closed: bool,
    }

    fn read_exact_or_end(stream: &mut UnixStream, buffer: &mut [u8]) -> io::Result<bool> {
        let mut read = 0;
        while read < buffer.len() {
            match stream.read(&mut buffer[read..])? {
                0 if read == 0 => return Ok(false),
                0 => return Err(io::Error::other("the observer closed mid-message")),
                n => read += n,
            }
        }
        Ok(true)
    }

    /// One handshake line, read byte by byte (nothing of a message that may
    /// follow it is consumed).
    fn read_line(stream: &mut UnixStream) -> io::Result<String> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        while !line.ends_with(b"\r\n") {
            if !read_exact_or_end(stream, &mut byte)? {
                return Err(io::Error::other("the observer closed mid-handshake"));
            }
            line.push(byte[0]);
            if line.len() > 4096 {
                return Err(io::Error::other("a handshake line too long"));
            }
        }
        line.truncate(line.len() - 2);
        String::from_utf8(line).map_err(io::Error::other)
    }

    /// One whole message, framed by its primary header and parsed by zbus;
    /// `None` at the end of the conversation.
    fn read_message(stream: &mut UnixStream) -> io::Result<Option<Message>> {
        let mut bytes = vec![0u8; 16];
        if !read_exact_or_end(stream, &mut bytes)? {
            return Ok(None);
        }
        let endian = match bytes[0] {
            b'l' => Endian::Little,
            b'B' => Endian::Big,
            other => return Err(io::Error::other(format!("endianness {other}"))),
        };
        let word = |at: usize| {
            let word: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
            match endian {
                Endian::Little => u32::from_le_bytes(word),
                Endian::Big => u32::from_be_bytes(word),
            }
        };
        let header = 16 + word(12) as usize;
        let total = header.div_ceil(8) * 8 + word(4) as usize;
        if total > 1024 * 1024 {
            return Err(io::Error::other("a message too long for a fixture"));
        }
        bytes.resize(total, 0);
        if !read_exact_or_end(stream, &mut bytes[16..])? {
            return Err(io::Error::other("the observer closed mid-message"));
        }
        // SAFETY: zbus parses and checks the message; it carries no
        // descriptors.
        let message =
            unsafe { Message::from_bytes(Data::new(bytes, Context::new_dbus(endian, 0))) }
                .map_err(io::Error::other)?;
        Ok(Some(message))
    }

    fn send(stream: &mut UnixStream, message: &Message) -> io::Result<()> {
        stream.write_all(message.data())
    }

    /// The bus side of one observation over `stream`.
    fn converse(
        mut stream: UnixStream,
        handshake: Handshake,
        answer: Option<Reply>,
    ) -> io::Result<Seen> {
        stream.set_read_timeout(Some(PEER_TIMEOUT))?;
        let mut seen = Seen::default();
        let mut nul = [0u8; 1];
        if !read_exact_or_end(&mut stream, &mut nul)? || nul[0] != 0 {
            return Err(io::Error::other("no leading NUL"));
        }
        seen.auth = read_line(&mut stream)?;
        match handshake {
            Handshake::Silent => {}
            Handshake::Reject => stream.write_all(b"REJECTED EXTERNAL\r\n")?,
            _ => {
                stream.write_all(format!("OK {GUID}\r\n").as_bytes())?;
                loop {
                    match read_line(&mut stream)?.as_str() {
                        "NEGOTIATE_UNIX_FD" => stream.write_all(b"AGREE_UNIX_FD\r\n")?,
                        "BEGIN" => break,
                        other => return Err(io::Error::other(format!("handshake line {other:?}"))),
                    }
                }
                let hello =
                    read_message(&mut stream)?.ok_or_else(|| io::Error::other("no Hello"))?;
                seen.messages.push(Sent::of(&hello));
                if handshake == Handshake::FailHello {
                    let refusal =
                        Message::method_error(&hello, "org.freedesktop.DBus.Error.Failed")
                            .map_err(io::Error::other)?
                            .build(&"no")
                            .map_err(io::Error::other)?;
                    send(&mut stream, &refusal)?;
                } else {
                    let name = Message::method_reply(&hello)
                        .map_err(io::Error::other)?
                        .build(&":1.42")
                        .map_err(io::Error::other)?;
                    send(&mut stream, &name)?;
                    if handshake == Handshake::CloseAfterHello {
                        return Ok(seen);
                    }
                    if let Some(call) = read_message(&mut stream)? {
                        seen.messages.push(Sent::of(&call));
                        if let Some(reply) = answer {
                            send(&mut stream, &reply(&call))?;
                        }
                    }
                }
            }
        }
        // Whatever else the observer sends is recorded; then its close.
        let mut rest = Vec::new();
        loop {
            match stream.read_to_end(&mut rest) {
                Ok(_) => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        if !rest.is_empty() && !seen.messages.is_empty() {
            return Err(io::Error::other(format!(
                "the observer sent {} more bytes",
                rest.len()
            )));
        }
        seen.closed = true;
        Ok(seen)
    }

    /// A fixture bus peer, served by one thread that is always joined.
    struct Peer {
        thread: Option<JoinHandle<io::Result<Seen>>>,
    }

    impl Peer {
        fn serve(stream: UnixStream, handshake: Handshake, answer: Option<Reply>) -> Self {
            Peer {
                thread: Some(std::thread::spawn(move || {
                    converse(stream, handshake, answer)
                })),
            }
        }

        /// The first connection to `listener` within the peer timeout, if
        /// any, then the conversation.
        fn accept(listener: UnixListener, handshake: Handshake, answer: Option<Reply>) -> Self {
            Peer {
                thread: Some(std::thread::spawn(move || {
                    let mut polled = [libc::pollfd {
                        fd: listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    }];
                    let millis = i32::try_from(PEER_TIMEOUT.as_millis()).unwrap();
                    // SAFETY: one initialized pollfd.
                    if unsafe { libc::poll(polled.as_mut_ptr(), 1, millis) } != 1 {
                        return Err(io::Error::other("no connection"));
                    }
                    let (stream, _) = listener.accept()?;
                    converse(stream, handshake, answer)
                })),
            }
        }

        fn seen(mut self) -> Seen {
            self.thread
                .take()
                .unwrap()
                .join()
                .expect("the peer thread")
                .expect("the peer's conversation")
        }
    }

    impl Drop for Peer {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// One observation against a fixture peer at the other end of a socket
    /// pair: its result, what the peer saw, and how long it took.
    fn observe_with(
        handshake: Handshake,
        answer: Option<Reply>,
        within: Duration,
    ) -> (Result<Scopes, ObservationError>, Seen, Duration) {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let peer = Peer::serve(theirs, handshake, answer);
        let started = Instant::now();
        let result = list_scopes(
            move || async move {
                ours.set_nonblocking(true)?;
                tokio::net::UnixStream::from_std(ours)
            },
            started + within,
        );
        let took = started.elapsed();
        (result, peer.seen(), took)
    }

    fn observe(answer: Option<Reply>) -> Result<Scopes, ObservationError> {
        let (result, seen, _) = observe_with(Handshake::Accept, answer, Duration::from_secs(10));
        assert!(seen.closed, "the observation released its connection");
        result
    }

    fn row(name: &str) -> UnitRow {
        (
            name.to_string(),
            "Nexus verifier execution".to_string(),
            "loaded".to_string(),
            "active".to_string(),
            "running".to_string(),
            String::new(),
            OwnedObjectPath::try_from(
                "/org/freedesktop/systemd1/unit/nexus_2dverifier_2d0a1b_2escope",
            )
            .unwrap(),
            0,
            String::new(),
            OwnedObjectPath::try_from("/").unwrap(),
        )
    }

    fn scopes(names: &[&str]) -> Scopes {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn hex(text: &str) -> String {
        text.bytes().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The exact request and nothing else: Hello, then the one listing call.
    fn assert_the_listing_request(seen: &Seen) {
        assert_eq!(
            seen.auth,
            format!("AUTH EXTERNAL {}", hex(&euid().to_string()))
        );
        let hello = Sent {
            kind: Type::MethodCall,
            destination: Some("org.freedesktop.DBus".to_string()),
            path: Some("/org/freedesktop/DBus".to_string()),
            interface: Some("org.freedesktop.DBus".to_string()),
            member: Some("Hello".to_string()),
            signature: None,
            arguments: None,
            fds: 0,
        };
        let listing = Sent {
            kind: Type::MethodCall,
            destination: Some("org.freedesktop.systemd1".to_string()),
            path: Some("/org/freedesktop/systemd1".to_string()),
            interface: Some("org.freedesktop.systemd1.Manager".to_string()),
            member: Some("ListUnitsByPatterns".to_string()),
            signature: Some("asas".to_string()),
            arguments: Some((Vec::new(), vec!["nexus-verifier-*.scope".to_string()])),
            fds: 0,
        };
        assert_eq!(seen.messages, [hello, listing]);
    }

    #[test]
    fn the_request_is_exactly_the_managers_listing_of_verifier_scopes() {
        let (result, seen, _) = observe_with(
            Handshake::Accept,
            answer_rows(Vec::new()),
            Duration::from_secs(10),
        );
        assert_eq!(result.unwrap(), Scopes::new());
        assert_the_listing_request(&seen);
        assert!(seen.closed, "the connection was released with the answer");
    }

    #[test]
    fn a_reply_lists_each_loaded_verifier_scope() {
        let mut inactive = row("nexus-verifier-ff00.scope");
        inactive.3 = "inactive".to_string();
        inactive.4 = "dead".to_string();
        let found = observe(answer_rows(vec![
            row("nexus-verifier-0a1b.scope"),
            inactive,
        ]))
        .unwrap();
        assert_eq!(
            found,
            scopes(&["nexus-verifier-0a1b.scope", "nexus-verifier-ff00.scope"])
        );
    }

    #[test]
    fn a_manager_error_is_an_error_never_no_scopes() {
        let result = observe(answer_error(
            "org.freedesktop.DBus.Error.AccessDenied",
            "Access denied".to_string(),
        ));
        match result {
            Err(ObservationError::Refused { name, message }) => {
                assert_eq!(name, "org.freedesktop.DBus.Error.AccessDenied");
                assert_eq!(message, "Access denied");
            }
            other => panic!("a refused listing is an error, never no scopes: {other:?}"),
        }
        // Its diagnostic is bounded, whatever the manager wrote.
        let result = observe(answer_error(
            "org.freedesktop.DBus.Error.Failed",
            format!("{}\n::error::injected", "x".repeat(10_000)),
        ));
        let shown = result.expect_err("an error").to_string();
        assert!(
            shown.chars().count() < 2 * MAX_DIAGNOSTIC,
            "{}",
            shown.len()
        );
        assert!(!shown.contains('\n'), "one line");
    }

    #[test]
    fn a_failed_handshake_or_hello_is_an_error() {
        for handshake in [Handshake::Reject, Handshake::FailHello] {
            let (result, seen, took) =
                observe_with(handshake, answer_rows(Vec::new()), Duration::from_secs(10));
            assert!(
                matches!(result, Err(ObservationError::Bus(_))),
                "{handshake:?}: {result:?}"
            );
            assert!(took < Duration::from_secs(5), "{handshake:?}: at once");
            assert!(seen.closed, "{handshake:?}: the connection was released");
        }
    }

    #[test]
    fn a_connection_closed_before_the_answer_is_an_error() {
        let (result, seen, took) = observe_with(
            Handshake::CloseAfterHello,
            answer_rows(Vec::new()),
            Duration::from_secs(10),
        );
        assert!(
            matches!(result, Err(ObservationError::Bus(_))),
            "{result:?}"
        );
        assert!(
            took < Duration::from_secs(5),
            "at once, not at the deadline"
        );
        assert_eq!(seen.messages.len(), 1, "Hello only");
    }

    #[test]
    fn an_observation_that_does_not_finish_times_out_and_releases_its_connection() {
        for (handshake, answer) in [
            // Reaching and authenticating: the bus never answers.
            (Handshake::Silent, answer_rows(Vec::new())),
            // The call: the manager never answers.
            (Handshake::Accept, None),
        ] {
            let within = Duration::from_millis(300);
            let (result, seen, took) = observe_with(handshake, answer, within);
            assert!(
                matches!(result, Err(ObservationError::Timeout)),
                "{handshake:?}: {result:?}"
            );
            assert!(
                took >= within && took < Duration::from_secs(5),
                "{handshake:?}: {took:?}"
            );
            assert!(seen.closed, "{handshake:?}: the connection was released");
        }
    }

    #[test]
    fn a_reply_of_another_shape_is_malformed() {
        type Wider = (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            OwnedObjectPath,
            u32,
            String,
            OwnedObjectPath,
        );
        let (name, description, load, active, sub, following, path, job, kind, job_path) =
            row("nexus-verifier-0a1b.scope");
        let wider: Vec<Wider> = vec![(
            name,
            description,
            load,
            active,
            sub,
            following,
            String::new(),
            path,
            job,
            kind,
            job_path,
        )];
        let answers: Vec<(&str, Option<Reply>)> = vec![
            (
                "names only",
                answer_body(vec!["nexus-verifier-0a1b.scope".to_string()]),
            ),
            ("a wider record", answer_body(wider)),
            ("no body", answer_body(())),
            (
                "the list and more",
                answer_body((vec![row("nexus-verifier-0a1b.scope")], 1u32)),
            ),
            (
                "undecodable records",
                Some(Box::new(|call: &Message| {
                    // SAFETY: deliberately not a valid body: an array that
                    // claims 16 bytes and holds 3.
                    unsafe {
                        Message::method_reply(call)
                            .unwrap()
                            .build_raw_body(&[16, 0, 0, 0, 1, 2, 3], REPLY_SIGNATURE, Vec::new())
                            .unwrap()
                    }
                })),
            ),
        ];
        for (what, answer) in answers {
            let result = observe(answer);
            assert!(
                matches!(result, Err(ObservationError::Malformed(_))),
                "{what}: {result:?}"
            );
        }
    }

    #[test]
    fn duplicate_or_out_of_pattern_units_are_malformed() {
        let lists: Vec<(&str, Vec<UnitRow>)> = vec![
            (
                "listed twice",
                vec![
                    row("nexus-verifier-0a1b.scope"),
                    row("nexus-verifier-0a1b.scope"),
                ],
            ),
            ("another unit", vec![row("other.scope")]),
            ("another type", vec![row("nexus-verifier-0a1b.service")]),
            ("no name", vec![row("nexus-verifier-.scope")]),
            ("a space", vec![row("nexus-verifier-0a 1b.scope")]),
            ("not ASCII", vec![row("nexus-verifier-\u{e9}.scope")]),
            ("a newline", vec![row("nexus-verifier-0a1b.scope\n")]),
        ];
        for (what, rows) in lists {
            let result = observe(answer_rows(rows));
            assert!(
                matches!(result, Err(ObservationError::Malformed(_))),
                "{what}: {result:?}"
            );
        }
    }

    #[test]
    fn every_bound_on_an_accepted_reply_is_enforced() {
        let named = |n: usize| -> Vec<UnitRow> {
            (0..n)
                .map(|i| row(&format!("nexus-verifier-{i:04x}.scope")))
                .collect()
        };
        assert_eq!(
            observe(answer_rows(named(MAX_UNITS))).unwrap().len(),
            MAX_UNITS
        );
        let result = observe(answer_rows(named(MAX_UNITS + 1)));
        assert!(
            matches!(&result, Err(ObservationError::Malformed(why)) if why.contains("more than 256")),
            "more than {MAX_UNITS} units are refused: {result:?}"
        );
        // A body over 64 KiB is refused before it is decoded, though every
        // record in it is within its own bounds.
        let long: Vec<UnitRow> = (0..80)
            .map(|i| {
                let mut unit = row(&format!("nexus-verifier-{i:04x}.scope"));
                unit.1 = "d".repeat(MAX_TEXT);
                unit
            })
            .collect();
        let result = observe(answer_rows(long));
        assert!(
            matches!(&result, Err(ObservationError::Malformed(why)) if why.contains("more than 65536")),
            "a body over {MAX_REPLY_BODY} bytes is refused before it is decoded: {result:?}"
        );
        let longest = format!("nexus-verifier-{}.scope", "a".repeat(MAX_NAME - 21));
        assert_eq!(longest.len(), MAX_NAME);
        assert_eq!(
            observe(answer_rows(vec![row(&longest)])).unwrap(),
            scopes(&[&longest])
        );
        let mut too_long = row(&format!(
            "nexus-verifier-{}.scope",
            "a".repeat(MAX_NAME - 20)
        ));
        let mut state = row("nexus-verifier-0a1b.scope");
        state.3 = "a".repeat(MAX_STATE + 1);
        let mut job = row("nexus-verifier-0a1b.scope");
        job.8 = "Start".to_string();
        let mut following = row("nexus-verifier-0a1b.scope");
        following.5 = "a b".to_string();
        let mut description = row("nexus-verifier-0a1b.scope");
        description.1 = "d".repeat(MAX_TEXT + 1);
        let mut unknown = row("nexus-verifier-0a1b.scope");
        unknown.2 = String::new();
        too_long.1 = "fits".to_string();
        for (what, unit) in [
            ("a name over 255 bytes", too_long),
            ("a state over 64 bytes", state),
            ("a job type not a state", job),
            ("a following unit not a unit name", following),
            ("a description over 1024 bytes", description),
            ("an empty load state", unknown),
        ] {
            let result = observe(answer_rows(vec![unit]));
            assert!(
                matches!(result, Err(ObservationError::Malformed(_))),
                "{what}: {result:?}"
            );
        }
    }

    #[test]
    fn the_bus_is_reached_only_through_the_checked_runtime_directory() {
        let root = Root::new("bus-ok");
        let (runtime, listener) = root.runtime(uid());
        let socket = fs::symlink_metadata(runtime.join("bus")).unwrap();
        let bus = user_bus(&root.host(uid(), None)).unwrap();
        assert_eq!(bus.identity().unwrap(), (socket.dev(), socket.ino()));
        let magic = filesystem_type(&runtime).unwrap();
        assert!(user_bus(&root.host(uid(), Some(magic))).is_ok());
        let peer = Peer::accept(
            listener,
            Handshake::Accept,
            answer_rows(vec![row("nexus-verifier-0a1b.scope")]),
        );
        let found = observe_scopes_on(
            &root.host(uid(), None),
            Instant::now() + Duration::from_secs(10),
        );
        let seen = peer.seen();
        assert_eq!(found.unwrap(), scopes(&["nexus-verifier-0a1b.scope"]));
        assert_the_listing_request(&seen);
        assert!(seen.closed);
        // The caller's deadline bounds the whole observation.
        let root = Root::new("bus-deadline");
        let (_runtime, listener) = root.runtime(uid());
        let peer = Peer::accept(listener, Handshake::Silent, None);
        let started = Instant::now();
        let result = observe_scopes_on(
            &root.host(uid(), None),
            started + Duration::from_millis(300),
        );
        assert!(
            matches!(result, Err(ObservationError::Timeout)),
            "{result:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(peer.seen().closed);
    }

    /// Whether anything connected to `listener` (without waiting).
    fn connected(listener: &UnixListener) -> bool {
        listener.set_nonblocking(true).unwrap();
        match listener.accept() {
            Ok(_) => true,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
            Err(error) => panic!("{error}"),
        }
    }

    #[test]
    fn an_unusable_runtime_directory_or_bus_is_an_error_and_nothing_is_reached() {
        type Change = fn(&Root, &Path);
        let changes: [(&str, Change); 10] = [
            ("no runtime directory", |_, runtime| {
                fs::remove_dir_all(runtime).unwrap()
            }),
            ("a symlinked runtime directory", |root, runtime| {
                let real = root.0.join("elsewhere");
                fs::rename(runtime, &real).unwrap();
                symlink(&real, runtime).unwrap();
            }),
            ("a shared runtime directory", |_, runtime| {
                mode(runtime, 0o755)
            }),
            ("no bus", |_, runtime| {
                fs::remove_file(runtime.join("bus")).unwrap()
            }),
            ("a bus that is a file", |_, runtime| {
                fs::remove_file(runtime.join("bus")).unwrap();
                fs::write(runtime.join("bus"), b"").unwrap();
            }),
            ("a symlinked bus", |root, runtime| {
                let real = root.0.join("real-bus");
                fs::rename(runtime.join("bus"), &real).unwrap();
                symlink(&real, runtime.join("bus")).unwrap();
            }),
            ("a writable /run", |root, _| {
                mode(&root.0.join("run"), 0o775)
            }),
            ("a writable /run/user", |root, _| {
                mode(&root.0.join("run/user"), 0o777)
            }),
            ("a symlinked /run", |root, _| {
                let real = root.0.join("real-run");
                fs::rename(root.0.join("run"), &real).unwrap();
                symlink(&real, root.0.join("run")).unwrap();
            }),
            ("a symlinked /run/user", |root, _| {
                let real = root.0.join("real-user");
                fs::rename(root.0.join("run/user"), &real).unwrap();
                symlink(&real, root.0.join("run/user")).unwrap();
            }),
        ];
        for (what, change) in changes {
            let root = Root::new("bus-bad");
            let (runtime, listener) = root.runtime(uid());
            change(&root, &runtime);
            let result = observe_scopes_on(
                &root.host(uid(), None),
                Instant::now() + Duration::from_secs(10),
            );
            assert!(
                matches!(result, Err(ObservationError::Runtime(_))),
                "{what}: {result:?}"
            );
            assert!(!connected(&listener), "{what}: nothing was reached");
        }
        // Another uid's runtime directory (owned here by this test), a
        // /run not owned by root's stand-in, and the wrong filesystem.
        let other = Root::new("bus-owner");
        let (_runtime, other_listener) = other.runtime(uid().wrapping_add(1));
        let root = Root::new("bus-root-owner");
        let (runtime, listener) = root.runtime(uid());
        let mut foreign = root.host(uid(), None);
        foreign.root_owner = uid().wrapping_add(1);
        let wrong = filesystem_type(&runtime).unwrap().wrapping_add(1);
        for host in [
            other.host(uid().wrapping_add(1), None),
            foreign,
            root.host(uid(), Some(wrong)),
        ] {
            let result = observe_scopes_on(&host, Instant::now() + Duration::from_secs(10));
            assert!(
                matches!(result, Err(ObservationError::Runtime(_))),
                "uid {} root {} magic {:?}: {result:?}",
                host.uid,
                host.root_owner,
                host.fs_magic
            );
        }
        assert!(!connected(&listener) && !connected(&other_listener));
    }

    #[test]
    fn poisoned_or_absent_ambient_environment_never_redirects_the_observation() {
        let ambient = [
            "DBUS_SESSION_BUS_ADDRESS",
            "DBUS_SYSTEM_BUS_ADDRESS",
            "DBUS_STARTER_ADDRESS",
            "XDG_RUNTIME_DIR",
            "HOME",
            "FLATPAK_ID",
        ];
        if std::env::var_os(AMBIENT_CHILD).is_some() {
            // Re-executed below, with that ambient environment: the
            // observation still goes only to the checked fixture bus, with
            // exactly the listing request.
            let root = Root::new("ambient");
            let (_runtime, listener) = root.runtime(uid());
            let peer = Peer::accept(listener, Handshake::Accept, answer_rows(Vec::new()));
            let result = observe_scopes_on(
                &root.host(uid(), None),
                Instant::now() + Duration::from_secs(10),
            );
            let seen = peer.seen();
            assert_eq!(result.unwrap(), Scopes::new());
            assert_the_listing_request(&seen);
            return;
        }
        for poisoned in [true, false] {
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "controls::poisoned_or_absent_ambient_environment_never_redirects_the_observation",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(AMBIENT_CHILD, "1");
            for name in ambient {
                if poisoned {
                    child.env(name, "unix:path=/nonexistent/poisoned/bus");
                } else {
                    child.env_remove(name);
                }
            }
            let output = child.output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(output.status.success(), "poisoned {poisoned}: {text}");
            assert!(text.contains("1 passed"), "poisoned {poisoned}: {text}");
        }
    }

    #[test]
    fn a_failed_observation_never_ends_a_wait_as_done() {
        let failing = |_| {
            Err(ObservationError::Refused {
                name: "org.freedesktop.DBus.Error.Failed".to_string(),
                message: "no".to_string(),
            })
        };
        let result = wait_for(Duration::from_secs(5), failing, |now| now.is_empty());
        assert!(
            matches!(result, Err(ObservationError::Refused { .. })),
            "{result:?}"
        );
        // A failure part-way through a wait ends it too.
        let mut answers = vec![
            Err(ObservationError::Malformed("x".into())),
            Ok(scopes(&["nexus-verifier-a.scope"])),
        ];
        let result = wait_for(
            Duration::from_secs(5),
            |_| answers.pop().unwrap(),
            |now| now.is_empty(),
        );
        assert!(
            matches!(result, Err(ObservationError::Malformed(_))),
            "{result:?}"
        );
        assert!(wait_for(
            Duration::from_secs(5),
            |_| Ok(Scopes::new()),
            |now| now.is_empty()
        )
        .unwrap());
        let started = Instant::now();
        let kept = scopes(&["nexus-verifier-a.scope"]);
        assert!(!wait_for(
            Duration::from_millis(100),
            |_| Ok(kept.clone()),
            |now| now.is_empty()
        )
        .unwrap());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_wait_gives_every_observation_its_callers_one_deadline() {
        let within = Duration::from_millis(200);
        let started = Instant::now();
        let deadlines = RefCell::new(Vec::new());
        let kept = scopes(&["nexus-verifier-a.scope"]);
        let done = wait_for(
            within,
            |deadline| {
                deadlines.borrow_mut().push(deadline);
                Ok(kept.clone())
            },
            |now| now.is_empty(),
        )
        .unwrap();
        let ended = Instant::now();
        assert!(!done);
        let deadlines = deadlines.into_inner();
        assert!(deadlines.len() > 1, "observed repeatedly");
        assert!(
            deadlines.iter().all(|deadline| *deadline == deadlines[0]),
            "never reset"
        );
        assert!(deadlines[0] >= started + within && deadlines[0] <= ended);
        // An observation is never given more than its own bound, whatever
        // the caller's deadline.
        let (ours, theirs) = UnixStream::pair().unwrap();
        let peer = Peer::serve(theirs, Handshake::Silent, None);
        let before = Instant::now();
        let result = list_scopes(
            move || async move {
                ours.set_nonblocking(true)?;
                tokio::net::UnixStream::from_std(ours)
            },
            before + Duration::from_millis(200),
        );
        assert!(matches!(result, Err(ObservationError::Timeout)));
        assert!(before.elapsed() < OBSERVATION_TIMEOUT);
        drop(peer);
    }

    /// `run/user/<uid>/nexus-verifier` in a fixture runtime directory.
    struct WorkspaceFixture {
        root: Root,
        runtime: PathBuf,
        _bus: UnixListener,
    }

    impl WorkspaceFixture {
        fn new(uid: u32) -> Self {
            let root = Root::new("ws");
            let (runtime, bus) = root.runtime(uid);
            WorkspaceFixture {
                root,
                runtime,
                _bus: bus,
            }
        }

        fn dir(&self) -> PathBuf {
            self.runtime.join(WORKSPACES)
        }

        fn private(self, entries: &[&str]) -> Self {
            fs::create_dir(self.dir()).unwrap();
            mode(&self.dir(), 0o700);
            for entry in entries {
                fs::create_dir(self.dir().join(entry)).unwrap();
            }
            self
        }

        fn observe(&self, fs: &Fs<'_>) -> Result<Workspaces, ObservationError> {
            observe_workspaces_with(&self.root.host(uid(), None), fs)
        }
    }

    fn listed(names: &[&str]) -> Workspaces {
        Workspaces::Listed(Listing {
            names: names.iter().map(|name| name.to_string()).collect(),
            more: false,
        })
    }

    /// `fstat`, except that its `n`th call (1-based) answers `change` of the
    /// real answer, or fails.
    fn nth_fstat(
        n: usize,
        change: impl Fn(&mut libc::stat) -> io::Result<()>,
    ) -> impl Fn(&OwnedFd) -> io::Result<libc::stat> {
        let calls = Cell::new(0);
        move |fd| {
            calls.set(calls.get() + 1);
            let mut st = fstat(fd)?;
            if calls.get() == n {
                change(&mut st)?;
            }
            Ok(st)
        }
    }

    fn fails(result: Result<Workspaces, ObservationError>, runtime: bool, what: &str) {
        let as_expected = if runtime {
            matches!(result, Err(ObservationError::Runtime(_)))
        } else {
            matches!(result, Err(ObservationError::Inspection(_)))
        };
        assert!(as_expected, "{what}: {result:?}");
    }

    #[test]
    fn an_empty_workspaces_directory_is_nothing_left() {
        let fixture = WorkspaceFixture::new(uid()).private(&[]);
        assert_eq!(fixture.observe(&Fs::real()).unwrap(), listed(&[]));
    }

    #[test]
    fn whatever_the_workspaces_directory_holds_is_left_behind() {
        let fixture = WorkspaceFixture::new(uid()).private(&["ws-0a1b"]);
        fs::write(fixture.dir().join(".hidden"), b"").unwrap();
        assert_eq!(
            fixture.observe(&Fs::real()).unwrap(),
            listed(&[".hidden", "ws-0a1b"])
        );
        // Bounded: never more than the bound is read, and the rest is
        // still "more".
        let fixture = WorkspaceFixture::new(uid()).private(&[]);
        for i in 0..MAX_LISTED + 5 {
            fs::write(fixture.dir().join(format!("ws-{i:03}")), b"").unwrap();
        }
        match fixture.observe(&Fs::real()).unwrap() {
            Workspaces::Listed(listing) => {
                assert_eq!(listing.names.len(), MAX_LISTED);
                assert!(listing.more && !listing.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn absence_is_only_a_missing_final_component_of_a_checked_runtime_directory() {
        let fixture = WorkspaceFixture::new(uid());
        assert_eq!(fixture.observe(&Fs::real()).unwrap(), Workspaces::Absent);
        assert!(!fixture.dir().exists(), "nothing was created");
        // A missing, symlinked or unchecked runtime directory is not
        // absence.
        let fixture = WorkspaceFixture::new(uid());
        fs::remove_dir_all(&fixture.runtime).unwrap();
        fails(fixture.observe(&Fs::real()), true, "no runtime directory");
        assert!(!fixture.runtime.exists(), "the parent was not created");
        let fixture = WorkspaceFixture::new(uid());
        let real = fixture.root.0.join("elsewhere");
        fs::rename(&fixture.runtime, &real).unwrap();
        symlink(&real, &fixture.runtime).unwrap();
        fails(
            fixture.observe(&Fs::real()),
            true,
            "a symlinked runtime directory",
        );
        let fixture = WorkspaceFixture::new(uid());
        mode(&fixture.root.0.join("run"), 0o777);
        fails(fixture.observe(&Fs::real()), true, "a writable /run");
        // Nor is a runtime directory removed while it is inspected (its
        // fourth stat: the check after the missing name).
        let fixture = WorkspaceFixture::new(uid());
        let removed = nth_fstat(4, |st| {
            st.st_nlink = 0;
            Ok(())
        });
        fails(
            fixture.observe(&Fs {
                fstat: &removed,
                ..Fs::real()
            }),
            false,
            "a runtime directory removed",
        );
    }

    #[test]
    fn a_symlink_or_another_type_is_never_empty() {
        type Change = fn(&WorkspaceFixture);
        let changes: [(&str, Change); 5] = [
            ("a symlink to a private directory", |f| {
                let real = f.runtime.join("real");
                fs::create_dir(&real).unwrap();
                mode(&real, 0o700);
                symlink(&real, f.dir()).unwrap();
            }),
            ("a dangling symlink", |f| {
                symlink("/nonexistent/nexus-p2v1r3a", f.dir()).unwrap()
            }),
            ("a file", |f| fs::write(f.dir(), b"").unwrap()),
            ("a FIFO", |f| {
                let path = CString::new(f.dir().as_os_str().as_bytes()).unwrap();
                // SAFETY: a NUL-terminated path.
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            }),
            ("a socket", |f| {
                drop(UnixListener::bind(f.dir()).unwrap());
            }),
        ];
        for (what, change) in changes {
            let fixture = WorkspaceFixture::new(uid());
            change(&fixture);
            fails(fixture.observe(&Fs::real()), false, what);
        }
    }

    #[test]
    fn a_wrong_owner_mode_or_filesystem_is_never_empty() {
        let fixture = WorkspaceFixture::new(uid()).private(&[]);
        mode(&fixture.dir(), 0o755);
        fails(
            fixture.observe(&Fs::real()),
            false,
            "a shared workspaces directory",
        );
        // Owners and filesystems this test cannot create are injected into
        // the workspaces directory's own stat (the fourth).
        let foreign = nth_fstat(4, |st| {
            st.st_uid = uid().wrapping_add(1);
            Ok(())
        });
        let elsewhere = nth_fstat(4, |st| {
            st.st_dev = st.st_dev.wrapping_add(1);
            Ok(())
        });
        type Stat<'a> = &'a dyn Fn(&OwnedFd) -> io::Result<libc::stat>;
        let injected: [(&str, Stat<'_>); 2] = [
            ("another owner", &foreign),
            ("another filesystem", &elsewhere),
        ];
        for (what, injected) in injected {
            let fixture = WorkspaceFixture::new(uid()).private(&[]);
            fails(
                fixture.observe(&Fs {
                    fstat: injected,
                    ..Fs::real()
                }),
                false,
                what,
            );
        }
        let other = Root::new("ws-other");
        let (runtime, _bus) = other.runtime(uid().wrapping_add(1));
        fs::create_dir(runtime.join(WORKSPACES)).unwrap();
        mode(&runtime.join(WORKSPACES), 0o700);
        fails(
            observe_workspaces(&other.host(uid().wrapping_add(1), None)),
            true,
            "another uid's runtime directory",
        );
        let fixture = WorkspaceFixture::new(uid()).private(&[]);
        fails(
            observe_workspaces(&fixture.root.host(uid(), Some(0x1234))),
            true,
            "the wrong filesystem",
        );
    }

    #[test]
    fn a_refused_or_failing_inspection_is_never_empty() {
        let refused = || io::Error::from_raw_os_error(libc::EACCES);
        let failing = || io::Error::from_raw_os_error(libc::EIO);
        let open_refused = |name: &'static str| {
            move |dir: &OwnedFd, entry: &str| -> io::Result<OwnedFd> {
                if entry == name {
                    Err(refused())
                } else {
                    open_dir_at(dir, entry)
                }
            }
        };
        let workspaces_refused = open_refused(WORKSPACES);
        let user_refused = open_refused("user");
        let listing_refused = |_: &OwnedFd, _: usize| -> io::Result<Listing> { Err(refused()) };
        let listing_failing = |_: &OwnedFd, _: usize| -> io::Result<Listing> { Err(failing()) };
        let stat_failing = nth_fstat(4, |_| Err(failing()));
        let runtime_stat_failing = nth_fstat(3, |_| Err(failing()));
        let cases: [(&str, bool, Fs<'_>); 6] = [
            (
                "an open refused",
                false,
                Fs {
                    open_dir_at: &workspaces_refused,
                    ..Fs::real()
                },
            ),
            (
                "a listing refused",
                false,
                Fs {
                    list: &listing_refused,
                    ..Fs::real()
                },
            ),
            (
                "a listing failing",
                false,
                Fs {
                    list: &listing_failing,
                    ..Fs::real()
                },
            ),
            (
                "a stat failing",
                false,
                Fs {
                    fstat: &stat_failing,
                    ..Fs::real()
                },
            ),
            (
                "/run/user refused",
                true,
                Fs {
                    open_dir_at: &user_refused,
                    ..Fs::real()
                },
            ),
            (
                "a runtime stat failing",
                true,
                Fs {
                    fstat: &runtime_stat_failing,
                    ..Fs::real()
                },
            ),
        ];
        for (what, runtime, fs) in cases {
            let fixture = WorkspaceFixture::new(uid()).private(&[]);
            fails(fixture.observe(&fs), runtime, what);
        }
    }

    /// The report of both observations, given stand-ins for each.
    fn outcome(
        scopes: Result<Scopes, ObservationError>,
        workspaces: Result<Workspaces, ObservationError>,
    ) -> (bool, String) {
        let calls = RefCell::new(Vec::new());
        let mut lines = Vec::new();
        let clean = report_cleanup(
            || {
                calls.borrow_mut().push("scopes");
                scopes
            },
            || {
                calls.borrow_mut().push("workspaces");
                workspaces
            },
            &mut |line| lines.push(line),
        );
        assert_eq!(
            calls.into_inner(),
            ["scopes", "workspaces"],
            "both observations always run"
        );
        (clean, lines.join("\n"))
    }

    #[test]
    fn only_two_answers_of_nothing_left_are_clean_and_neither_failure_hides_the_other() {
        let refused = || ObservationError::Refused {
            name: "org.freedesktop.DBus.Error.AccessDenied".to_string(),
            message: "denied".to_string(),
        };
        assert!(outcome(Ok(Scopes::new()), Ok(listed(&[]))).0);
        let (clean, report) = outcome(Ok(Scopes::new()), Ok(Workspaces::Absent));
        assert!(clean);
        assert!(report.contains("absent at observation time"), "{report}");
        assert!(!report.contains("never created"), "{report}");
        for (scopes, workspaces, expected) in [
            (
                Err(refused()),
                Ok(listed(&[])),
                "::error::the verifier scope observation failed: the user manager refused",
            ),
            (
                Ok(Scopes::new()),
                Err(ObservationError::Inspection("cannot be listed".to_string())),
                "::error::the verification workspace observation failed: cannot be listed",
            ),
            (
                Ok(scopes(&["nexus-verifier-0a1b.scope"])),
                Ok(listed(&[])),
                "::error::a verifier scope was left behind: nexus-verifier-0a1b.scope",
            ),
            (
                Ok(Scopes::new()),
                Ok(listed(&["ws-0a1b"])),
                "::error::a verification workspace was left behind: \"ws-0a1b\"",
            ),
        ] {
            let (clean, report) = outcome(scopes, workspaces);
            assert!(!clean, "{report}");
            assert!(report.contains(expected), "{report}");
        }
        let (clean, report) = outcome(Err(ObservationError::Timeout), Err(refused()));
        assert!(!clean);
        assert_eq!(report.matches("::error::").count(), 2, "{report}");
    }

    #[test]
    fn a_panicking_observation_is_a_failure_and_the_other_still_runs() {
        let calls = RefCell::new(Vec::new());
        let mut lines = Vec::new();
        let clean = report_cleanup(
            || -> Result<Scopes, ObservationError> {
                calls.borrow_mut().push("scopes");
                panic!("injected")
            },
            || {
                calls.borrow_mut().push("workspaces");
                Ok(listed(&[]))
            },
            &mut |line| lines.push(line),
        );
        assert!(!clean);
        assert_eq!(calls.into_inner(), ["scopes", "workspaces"]);
        assert!(
            lines[0].contains(
                "::error::the verifier scope observation failed: the observation panicked"
            ),
            "{lines:?}"
        );
    }

    /// What a stand-in entry was asked to be.
    #[derive(Debug, PartialEq)]
    enum Entered {
        Suite,
        Observation,
        Mode(Mode, Vec<String>),
        Refused(Refusal),
    }

    /// Stand-ins for every entry: nothing real ever runs.
    struct StandIn;

    impl Entry for StandIn {
        type Exit = Entered;

        fn suite(&mut self) -> Entered {
            Entered::Suite
        }

        fn cleanup_observation(&mut self) -> Entered {
            Entered::Observation
        }

        fn mode(&mut self, mode: Mode, args: &[String]) -> Entered {
            Entered::Mode(mode, args.to_vec())
        }

        fn refuse(&mut self, refusal: Refusal) -> Entered {
            Entered::Refused(refusal)
        }
    }

    fn entered(args: &[&str], supported: bool) -> Entered {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        dispatch(&args, supported, &mut StandIn)
    }

    #[test]
    fn the_observation_mode_and_invalid_arguments_never_enter_the_suite() {
        assert_eq!(entered(&["harness"], true), Entered::Suite);
        assert_eq!(
            entered(&["harness", "--cleanup-observation"], true),
            Entered::Observation
        );
        assert_eq!(entered(&[], true), Entered::Refused(Refusal::NoProgram));
        for args in [
            &["harness", "--cleanup-observation", "/run/user/0/bus"][..],
            &["harness", "--cleanup-observation", "--cleanup-observation"],
            &["harness", "--cleanup-observation", ""],
        ] {
            assert_eq!(
                entered(args, true),
                Entered::Refused(Refusal::ObservationArguments(1)),
                "{args:?}"
            );
        }
        for unknown in [
            "",
            "-",
            "--",
            "--cleanup-observation=1",
            "--CLEANUP-OBSERVATION",
            "--suite",
            "--help",
            "--nocapture",
            "--test-threads=1",
            "--exact",
            "probe",
            "--PROBE",
            "p2d_live_execution_runs_in_a_verified_scope",
        ] {
            assert_eq!(
                entered(&["harness", unknown, "x"], true),
                Entered::Refused(Refusal::Unknown(unknown.to_string())),
                "{unknown:?}"
            );
        }
        let not_utf8 = vec![OsString::from("harness"), OsString::from_vec(vec![0xff])];
        assert_eq!(
            dispatch(&not_utf8, true, &mut StandIn),
            Entered::Refused(Refusal::NotUtf8)
        );
        // Only no argument at all enters the suite.
        for args in [
            &["harness", "--cleanup-observation"][..],
            &["harness", "x"],
            &["harness", "--probe"],
            &["harness", "", ""],
            &[],
        ] {
            assert_ne!(entered(args, true), Entered::Suite, "{args:?}");
        }
        // A refusal's description is bounded, whatever the argument.
        let shown = Refusal::Unknown("x\n".repeat(10_000)).to_string();
        assert!(shown.chars().count() < 2 * MAX_DIAGNOSTIC && !shown.contains('\n'));
    }

    #[test]
    fn each_probe_or_driver_mode_is_chosen_by_its_own_first_argument() {
        for (flag, mode) in Mode::ALL {
            assert_eq!(
                entered(&["probe", flag, "only=noop", "marker=x"], true),
                Entered::Mode(mode, vec!["only=noop".to_string(), "marker=x".to_string()]),
                "{flag}"
            );
            assert_eq!(
                entered(&["probe", flag], true),
                Entered::Mode(mode, Vec::new())
            );
        }
    }

    #[test]
    fn where_the_sandbox_does_not_exist_only_the_suites_note_remains() {
        assert_eq!(entered(&["harness"], false), Entered::Suite);
        assert_eq!(
            entered(&["harness", "--cleanup-observation"], false),
            Entered::Refused(Refusal::Unsupported("--cleanup-observation".to_string()))
        );
        for (flag, _) in Mode::ALL {
            assert_eq!(
                entered(&["harness", flag], false),
                Entered::Refused(Refusal::Unsupported(flag.to_string()))
            );
        }
        assert_eq!(
            entered(&["harness", "--suite"], false),
            Entered::Refused(Refusal::Unknown("--suite".to_string()))
        );
    }

    /// The shared observation's source, comments aside.
    fn observer_code() -> String {
        include_str!("support/cleanup_observation.rs")
            .lines()
            .map(|line| line.find("//").map_or(line, |at| &line[..at]))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_observer_has_no_process_path_and_reads_no_environment() {
        let code = observer_code();
        for needle in [
            "Command",
            "process::Child",
            "process::Stdio",
            "spawn",
            "fork",
            "exec",
            "kill",
            "waitid",
            "waitpid",
            "pidfd",
            "systemctl",
            "busctl",
            "env::var",
            "var_os",
            "set_var",
            "Builder::address",
            "Builder::session",
            "Builder::system",
            "Connection::session",
            "Connection::system",
            "add_match",
            "AddMatch",
            "Subscribe",
            "StopUnit",
            "KillUnit",
            "ResetFailed",
        ] {
            assert!(!code.contains(needle), "{needle}");
        }
        // One call, to one method of the manager, through one
        // authentication mechanism, under one deadline.
        assert_eq!(code.matches(".call_method(").count(), 1);
        assert_eq!(code.matches("\"ListUnitsByPatterns\"").count(), 1);
        assert_eq!(
            code.matches(".auth_mechanism(zbus::AuthMechanism::External)")
                .count(),
            1
        );
        assert_eq!(code.matches("timeout_at(").count(), 1);
    }

    /// An owner standing in for a retained boundary: its cleanup fails
    /// `failures` more times, every attempt is counted, and its drop (the
    /// end of its ownership) is witnessed.
    #[derive(Debug)]
    struct Witness {
        id: usize,
        failures: usize,
        tries: Rc<Cell<usize>>,
        dropped: Rc<Cell<usize>>,
    }

    impl Witness {
        fn new(failures: usize) -> (Self, Rc<Cell<usize>>, Rc<Cell<usize>>) {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let (tries, dropped) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
            let witness = Witness {
                id: NEXT.fetch_add(1, Ordering::SeqCst),
                failures,
                tries: tries.clone(),
                dropped: dropped.clone(),
            };
            (witness, tries, dropped)
        }

        /// As a retained boundary's retry: a confirmation consumes the
        /// owner; a failure returns the very same owner.
        fn retry(mut self) -> Result<(), Self> {
            self.tries.set(self.tries.get() + 1);
            if self.failures == 0 {
                return Ok(());
            }
            self.failures -= 1;
            Err(self)
        }
    }

    impl Drop for Witness {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }

    fn before_and_kept() -> (Scopes, Scopes) {
        (
            scopes(&["nexus-verifier-a.scope"]),
            scopes(&["nexus-verifier-a.scope", "nexus-verifier-b.scope"]),
        )
    }

    #[test]
    fn each_failed_attempt_returns_the_same_owner() {
        let (owner, tries, dropped) = Witness::new(2);
        let id = owner.id;
        let settled = settle(owner, EXPLICIT_ATTEMPTS, |owner: Witness| {
            assert_eq!(owner.id, id, "the same owner");
            assert_eq!(dropped.get(), 0, "never dropped between attempts");
            owner.retry()
        });
        assert!(matches!(settled, Settled::Confirmed(3)), "{settled:?}");
        assert_eq!((tries.get(), dropped.get()), (3, 1));
    }

    #[test]
    fn a_failed_retry_keeps_its_owner_for_a_later_explicit_retry() {
        let (owner, tries, dropped) = Witness::new(usize::MAX);
        let id = owner.id;
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(
            dropped.get(),
            0,
            "the owner is never dropped by a failed cleanup: {verdict:?}"
        );
        let Verdict::Unconfirmed {
            mut owner,
            failures,
        } = verdict
        else {
            panic!("an unconfirmed cleanup keeps its owner: {verdict:?}");
        };
        assert_eq!(owner.id, id, "the very owner every failed attempt returned");
        assert_eq!(tries.get(), EXPLICIT_ATTEMPTS);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("still unconfirmed after 3 explicit attempts")),
            "{failures:?}"
        );
        // A later explicit retry confirms the cleanup, ending the ownership.
        owner.failures = 0;
        assert!(matches!(
            settle(owner, 1, Witness::retry),
            Settled::Confirmed(1)
        ));
        assert_eq!((tries.get(), dropped.get()), (EXPLICIT_ATTEMPTS + 1, 1));
    }

    #[test]
    fn an_observation_failure_survives_a_confirming_retry() {
        // A real failed observation: the manager refused the listing.
        let observed = observe(answer_error(
            "org.freedesktop.DBus.Error.Failed",
            "No medium found".to_string(),
        ))
        .map_err(|error| error.to_string());
        let (owner, tries, dropped) = Witness::new(0);
        let (before, _) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            observed,
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(
            (tries.get(), dropped.get()),
            (1, 1),
            "the retry ran and confirmed"
        );
        let Verdict::Failed(failures) = verdict else {
            panic!("an observation failure stays a failure: {verdict:?}");
        };
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("could not be observed")
                    && failure.contains("No medium found")),
            "{failures:?}"
        );
    }

    #[test]
    fn observation_and_cleanup_failures_are_kept_together_with_the_owner() {
        let (owner, _, dropped) = Witness::new(usize::MAX);
        let (before, _) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Err("the query failed".to_string()),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(
            dropped.get(),
            0,
            "the owner is never dropped by a failed cleanup: {verdict:?}"
        );
        let Verdict::Unconfirmed { owner, failures } = verdict else {
            panic!("an unconfirmed cleanup keeps its owner: {verdict:?}");
        };
        assert!(failures
            .iter()
            .any(|failure| failure.contains("the query failed")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("still unconfirmed")));
        // Released explicitly, once reported: the report keeps both.
        let report = release(owner, "case", &failures);
        assert_eq!(dropped.get(), 1);
        assert!(report.contains("the query failed") && report.contains("still unconfirmed"));
    }

    #[test]
    fn evidence_is_judged_only_after_the_cleanup() {
        let (owner, tries, dropped) = Witness::new(0);
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            vec!["the retained tree is not alive".to_string()],
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!((tries.get(), dropped.get()), (1, 1), "cleaned up first");
        let Verdict::Failed(failures) = verdict else {
            panic!("{verdict:?}");
        };
        assert_eq!(failures, ["the retained tree is not alive"]);
    }

    #[test]
    fn a_cleanup_confirmed_only_on_a_later_attempt_still_fails() {
        let (owner, tries, dropped) = Witness::new(1);
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!((tries.get(), dropped.get()), (2, 1));
        let Verdict::Failed(failures) = verdict else {
            panic!("{verdict:?}");
        };
        assert!(
            failures[0].contains("confirmed only on explicit attempt 2"),
            "{failures:?}"
        );
    }

    #[test]
    fn a_kept_scope_and_a_first_confirmed_cleanup_pass() {
        let (owner, tries, dropped) = Witness::new(0);
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert!(matches!(verdict, Verdict::Passed), "{verdict:?}");
        assert_eq!((tries.get(), dropped.get()), (1, 1));
        // Not kept: a failure, after the cleanup.
        let (owner, _, dropped) = Witness::new(0);
        let verdict = judge_retained(
            Vec::new(),
            Ok(before.clone()),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(dropped.get(), 1);
        assert!(
            matches!(&verdict, Verdict::Failed(failures) if failures[0].contains("not kept")),
            "{verdict:?}"
        );
    }
}
