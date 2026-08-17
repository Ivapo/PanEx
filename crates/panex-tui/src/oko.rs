//! What every tab in this iTerm2 window is doing, read from `oko --follow`.
//!
//! A child process and a serialized stream rather than a crate dependency,
//! because the feature has to degrade to nothing when oko is absent — and a
//! compile-time dependency is never absent. The whole contract is that a
//! binary named `oko` may be on `PATH`, and what its `rules/follow-stream.md`
//! documents. See Ivapo/PanEx#2.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// The one stream schema this build knows how to draw. A header announcing
/// anything else means drawing nothing and saying so, rather than rendering
/// the fields we happen to recognise: a card that is confidently wrong is
/// worse than one that is absent.
const KNOWN_SCHEMA: u32 = 1;

#[derive(Deserialize)]
struct Header {
    schema: u32,
}

#[derive(Deserialize)]
struct SnapshotLine {
    rows: Vec<Row>,
}

/// One session in oko's window. Names and nullability are oko's, and so is
/// the rule that `status` and `job` are exclusive — a row carrying a status
/// is a Claude tab and publishes no job name.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Row {
    /// The iTerm2 session UUID: the join key, and what the card is keyed on.
    /// Selection follows this rather than a position, so a row appearing or
    /// closing cannot re-point a jump or a rename at a neighbour.
    pub session_id: String,
    pub tab: u32,
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub age: Option<String>,
    #[serde(default)]
    pub job: Option<String>,
}

/// What the reader thread has to say. `Lost` is terminal — the stream that
/// produced it has ended, and the message is for the user.
#[derive(Debug, PartialEq)]
pub enum Event {
    Rows(Vec<Row>),
    Lost(String),
}

/// What the card view is currently showing.
pub enum View {
    /// Spawned, no snapshot yet. A failed connect writes nothing to stdout at
    /// all, so this is also what an unreachable iTerm2 API looks like until
    /// the child exits and the reason arrives on stderr.
    Connecting,
    Rows(Vec<Row>),
    Lost(String),
}

/// How long the probe waits before calling it a no. A build that answers
/// `--version` answers in milliseconds; one that goes off to open an iTerm2
/// connection first is the stale build this is here to detect, and there is
/// nothing to gain by waiting for it to finish failing.
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);

/// Whether a *usable* oko is on `PATH`, decided once per process.
///
/// Presence is not capability. A build predating `--follow` answers that flag
/// by falling through to its own dashboard, where `ratatui::init()` meets a
/// pipe and panics — so the caller learns nothing from the file existing.
/// Asking for a version is unambiguous, and `PATH` does not change under a
/// running process, so the answer is worth keeping.
pub fn is_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(probe)
}

fn probe() -> bool {
    let Ok(mut child) = Command::new("oko")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false; // not on PATH at all, which costs nothing to discover
    };

    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Ask oko to jump iTerm2's focus to a session.
pub fn activate(session_id: &str) -> Result<(), String> {
    run_command(&["--activate", session_id])
}

/// Name a session, or clear the name with `None` — which is the only way back
/// to the derived default. Not an empty string: oko reads that back as a name
/// and renders it blank, with no further rename able to escape it.
pub fn set_name(session_id: &str, name: Option<&str>) -> Result<(), String> {
    match name {
        Some(name) => run_command(&["--set-name", session_id, name]),
        None => run_command(&["--set-name", session_id]),
    }
}

/// One short-lived oko, run to completion. These are rare and immediate, so
/// they are waited on rather than handed to the reader thread — the stream
/// carries the result back on its own when the change lands.
fn run_command(args: &[&str]) -> Result<(), String> {
    let output = Command::new("oko")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run oko: {}", e))?;

    if output.status.success() {
        return Ok(());
    }
    // oko puts one line on stderr and exits non-zero. A stale session id — a
    // tab closed since the card was drawn — arrives here as BadIdentifier.
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(match stderr.trim().lines().last() {
        Some(line) if !line.is_empty() => line.to_string(),
        _ => "oko failed".to_string(),
    })
}

/// A running `oko --follow` and the thread draining it.
pub struct Stream {
    child: Child,
    events: Receiver<Event>,
}

impl Stream {
    pub fn start() -> Result<Self, String> {
        let mut child = Command::new("oko")
            .arg("--follow")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not start oko: {}", e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "oko gave no stdout to read".to_string())?;
        let stderr = child.stderr.take();

        let (tx, events) = mpsc::channel();
        thread::spawn(move || read_stream(stdout, stderr, &tx));

        Ok(Self { child, events })
    }

    /// Everything queued since the last call, oldest first.
    pub fn drain(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // oko exits on its own once stdout closes, but a card view that has
        // been closed should not leave an iTerm2 connection open on the
        // chance that it does.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_stream(stdout: ChildStdout, stderr: Option<ChildStderr>, tx: &Sender<Event>) {
    let mut lines = BufReader::new(stdout).lines();

    // The header arrives once per stream, before anything else. What turns up
    // instead is either an escape sequence from a build that does not know
    // `--follow`, or — after a failed connect — nothing at all.
    let header = loop {
        match lines.next() {
            Some(Ok(line)) if line.trim().is_empty() => continue,
            Some(Ok(line)) => break serde_json::from_str::<Header>(&line).ok(),
            Some(Err(_)) | None => break None,
        }
    };

    let Some(header) = header else {
        let _ = tx.send(Event::Lost(last_words(stderr)));
        return;
    };

    if header.schema != KNOWN_SCHEMA {
        let _ = tx.send(Event::Lost(format!(
            "oko speaks stream schema {}; this build draws {}",
            header.schema, KNOWN_SCHEMA
        )));
        return;
    }

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        // A keepalive is a bare newline every five seconds — proof the pipe is
        // open, with nothing to draw. Handing it to a parser fails on a timer.
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<SnapshotLine>(&line) {
            Ok(snapshot) => {
                if tx.send(Event::Rows(snapshot.rows)).is_err() {
                    return; // the view is gone; Drop kills the child
                }
            }
            Err(e) => {
                let _ = tx.send(Event::Lost(format!("unreadable line from oko: {}", e)));
                return;
            }
        }
    }

    let _ = tx.send(Event::Lost(last_words(stderr)));
}

/// oko reports a failed connect on stderr and writes nothing to stdout, so
/// once the stream has ended empty that is the only place a reason exists.
fn last_words(stderr: Option<ChildStderr>) -> String {
    let mut buf = String::new();
    if let Some(mut handle) = stderr {
        let _ = handle.read_to_string(&mut buf);
    }
    match buf.trim().lines().last() {
        Some(line) if !line.is_empty() => line.to_string(),
        _ => "oko stopped".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Feeds the reader canned stdout, as if it were the child's pipe.
    fn events_from(stdout: &str) -> Vec<Event> {
        let (tx, rx) = mpsc::channel();
        // A pipe whose writer is already gone reads the bytes then EOFs, which
        // is the shape read_stream expects without needing a real child.
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdout.as_bytes())
            .unwrap();
        let out = child.stdout.take().unwrap();
        read_stream(out, None, &tx);
        let _ = child.wait();
        rx.try_iter().collect()
    }

    const HEADER: &str = r#"{"oko":"0.1.0","schema":1}"#;
    const SNAPSHOT: &str = r#"{"rows":[{"age":">30m","claude":true,"name":"oko","path":"/x/oko","session_id":"A","status":"ready","tab":2}],"window_number":0}"#;

    #[test]
    fn reads_a_snapshot_after_the_header() {
        let events = events_from(&format!("{HEADER}\n{SNAPSHOT}\n"));
        assert_eq!(
            events[0],
            Event::Rows(vec![Row {
                session_id: "A".into(),
                tab: 2,
                name: Some("oko".into()),
                path: Some("/x/oko".into()),
                status: Some("ready".into()),
                age: Some(">30m".into()),
                job: None,
            }])
        );
    }

    /// The keepalive is a bare newline every five seconds. Parsing it would
    /// tear the stream down on a timer.
    #[test]
    fn keepalives_are_not_snapshots() {
        let events = events_from(&format!("{HEADER}\n\n{SNAPSHOT}\n\n\n"));
        let rows = events
            .iter()
            .filter(|e| matches!(e, Event::Rows(_)))
            .count();
        assert_eq!(rows, 1, "only the real snapshot should draw: {events:?}");
    }

    /// A row without a status is a plain tab and carries a job name instead.
    #[test]
    fn a_plain_tab_carries_a_job_and_no_status() {
        let line = r#"{"rows":[{"age":null,"job":"panex","name":"ivapo","path":"/Users/me","session_id":"B","status":null,"tab":1}],"window_number":0}"#;
        let events = events_from(&format!("{HEADER}\n{line}\n"));
        match &events[0] {
            Event::Rows(rows) => {
                assert_eq!(rows[0].job.as_deref(), Some("panex"));
                assert!(rows[0].status.is_none());
            }
            other => panic!("expected rows, got {other:?}"),
        }
    }

    /// Drawing the fields we recognise out of a schema we do not know is the
    /// confidently-wrong card the contract forbids.
    #[test]
    fn an_unknown_schema_draws_nothing() {
        let events = events_from("{\"oko\":\"9.9.9\",\"schema\":7}\n");
        match &events[0] {
            Event::Lost(msg) => assert!(msg.contains('7'), "should name the schema: {msg}"),
            other => panic!("expected a loss, got {other:?}"),
        }
    }

    /// What a build predating `--follow` puts on the pipe before it dies.
    #[test]
    fn a_stale_oko_is_a_loss_not_a_card() {
        let events = events_from("\u{1b}[?1049l");
        assert!(matches!(events[0], Event::Lost(_)), "got {events:?}");
    }

    /// A failed connect writes nothing at all, and the view must not sit on
    /// "connecting" forever waiting for a header that is never coming.
    #[test]
    fn an_empty_stream_is_a_loss() {
        let events = events_from("");
        assert_eq!(events, vec![Event::Lost("oko stopped".to_string())]);
    }

    /// Against a real `oko --follow`, which the canned tests above cannot
    /// check: that the flag is spelled right, that the probe agrees with what
    /// the stream then does, and that a snapshot arrives without being asked.
    ///
    /// Ignored by default — it needs a usable oko on PATH and a live iTerm2
    /// window, neither of which a test run can assume. Run it with
    /// `cargo test -p panex-tui -- --ignored live_oko`.
    #[test]
    #[ignore]
    fn live_oko_produces_a_snapshot() {
        assert!(is_available(), "no usable oko on PATH");
        let stream = Stream::start().expect("stream should start");

        // The first event settles it either way, so nothing after it matters.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match stream.drain().into_iter().next() {
                Some(Event::Rows(rows)) => {
                    assert!(!rows.is_empty(), "a live window has at least this tab");
                    return;
                }
                Some(Event::Lost(message)) => panic!("stream lost: {message}"),
                None => {
                    assert!(Instant::now() < deadline, "no snapshot within 10s");
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
}
