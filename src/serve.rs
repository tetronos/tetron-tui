//! Share the shown invite on the LAN (TUI-SERVE-001). The TUI serves nothing
//! itself: it runs core contrib `tetron-invite-serve.py --invite-stdin` as a
//! hidden child process, reads the URL from its first stdout line and kills
//! it when the user stops sharing.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::ActionResult;

const SCRIPT: &str = "tetron-invite-serve.py";
const START_LIMIT: Duration = Duration::from_secs(10);

/// Where the script is looked for, in order: $TETRON_INVITE_SERVE, $PATH,
/// /usr/share/tetron, /usr/local/share/tetron.
pub fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = std::env::var_os("TETRON_INVITE_SERVE") {
        v.push(PathBuf::from(p));
    }
    if let Some(path) = std::env::var_os("PATH") {
        v.extend(std::env::split_paths(&path).map(|d| d.join(SCRIPT)));
    }
    v.push(Path::new("/usr/share/tetron").join(SCRIPT));
    v.push(Path::new("/usr/local/share/tetron").join(SCRIPT));
    v
}

/// The first existing candidate, or an error naming the file and where it was
/// looked for.
pub fn find_script() -> Result<PathBuf, String> {
    first_existing(candidates())
}

fn first_existing(c: Vec<PathBuf>) -> Result<PathBuf, String> {
    c.into_iter().find(|p| p.is_file()).ok_or_else(|| {
        format!(
            "share: {SCRIPT} not found (set TETRON_INVITE_SERVE, or install it in PATH or /usr/share/tetron)"
        )
    })
}

/// python3 arguments: isolated mode, the script, the network, invite on stdin.
pub fn args(script: &Path, network: &str) -> Vec<String> {
    ["-I", &script.display().to_string(), network, "--invite-stdin"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// A short reason for a child that ended after it printed its URL.
pub fn end_reason(code: Option<i32>) -> String {
    match code {
        Some(3) => "too many wrong requests".into(),
        _ => "stopped".into(),
    }
}

/// Run the child until it ends or `stop` fires. Reports [`ActionResult`]s
/// tagged with the invite `id`.
pub async fn run(
    python: &str,
    script: PathBuf,
    network: String,
    code: String,
    id: String,
    tx: UnboundedSender<ActionResult>,
    stop: Arc<Notify>,
) {
    let err = |msg: String| {
        let _ = tx.send(ActionResult::ShareEnded { id: id.clone(), reason: msg, failed: true });
    };
    let mut child = match Command::new(python)
        .args(args(&script, &network))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return err(format!("share: start {python}: {e}")),
    };
    if let Some(mut si) = child.stdin.take() {
        let line = format!("{code}\n");
        if si.write_all(line.as_bytes()).await.is_err() {
            return err("share: could not hand the invite to the script".into());
        }
    }
    let mut out = BufReader::new(child.stdout.take().expect("piped"));
    let mut first = String::new();
    let got = tokio::time::timeout(START_LIMIT, out.read_line(&mut first)).await;
    let url = first.trim().to_string();
    if !matches!(got, Ok(Ok(n)) if n > 0) || url.is_empty() {
        let _ = child.kill().await;
        let mut msg = String::new();
        if let Some(mut se) = child.stderr.take() {
            let _ = tokio::time::timeout(Duration::from_secs(1), se.read_to_string(&mut msg)).await;
        }
        let msg = msg.lines().last().unwrap_or("no URL printed").trim().to_string();
        return err(format!("share: {msg}"));
    }
    let _ = tx.send(ActionResult::Sharing { id: id.clone(), url });
    tokio::select! {
        st = child.wait() => {
            let code = st.ok().and_then(|s| s.code());
            let _ = tx.send(ActionResult::ShareEnded { id, reason: end_reason(code), failed: false });
        }
        _ = stop.notified() => {
            let _ = child.kill().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    fn stub(tag: &str, body: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tetron-tui-serve-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(SCRIPT);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn args_isolated_network_invite_on_stdin() {
        let a = args(Path::new("/x/s.py"), "net");
        assert_eq!(a, ["-I", "/x/s.py", "net", "--invite-stdin"]);
        assert!(!a.iter().any(|s| s.contains("INVITE")));
    }

    #[test]
    fn reasons() {
        assert_eq!(end_reason(Some(3)), "too many wrong requests");
        assert_eq!(end_reason(Some(0)), "stopped");
        assert_eq!(end_reason(None), "stopped");
    }

    #[test]
    fn missing_script_names_file_and_places() {
        let e = first_existing(vec![PathBuf::from("/nonexistent/x")]).unwrap_err();
        assert!(e.contains(SCRIPT) && e.contains("/usr/share/tetron") && e.contains("TETRON_INVITE_SERVE"), "{e}");
    }

    #[test]
    fn first_existing_candidate_wins() {
        let a = stub("c1", "");
        assert_eq!(first_existing(vec![PathBuf::from("/nonexistent/x"), a.clone()]).unwrap(), a);
        assert_eq!(candidates().last().unwrap(), &Path::new("/usr/local/share/tetron").join(SCRIPT));
    }

    #[tokio::test]
    async fn url_then_stop_kills_the_child() {
        let s = stub("stop", "import sys,time\nsys.stdin.readline()\nprint('http://1.2.3.4:7979/abc234',flush=True)\ntime.sleep(60)\n");
        let (tx, mut rx) = unbounded_channel();
        let stop = Arc::new(Notify::new());
        let t = tokio::spawn(run("python3", s, "n".into(), "CODE".into(), "i1".into(), tx, stop.clone()));
        match rx.recv().await.unwrap() {
            ActionResult::Sharing { id, url } => {
                assert_eq!((id.as_str(), url.as_str()), ("i1", "http://1.2.3.4:7979/abc234"));
            }
            _ => panic!("expected Sharing"),
        }
        stop.notify_one();
        tokio::time::timeout(Duration::from_secs(5), t).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn lockout_exit_3_is_reported() {
        let s = stub("lock", "import sys\nsys.stdin.readline()\nprint('http://h/x',flush=True)\nsys.exit(3)\n");
        let (tx, mut rx) = unbounded_channel();
        tokio::spawn(run("python3", s, "n".into(), "C".into(), "i2".into(), tx, Arc::new(Notify::new())));
        assert!(matches!(rx.recv().await.unwrap(), ActionResult::Sharing { .. }));
        match rx.recv().await.unwrap() {
            ActionResult::ShareEnded { reason, failed, .. } => {
                assert_eq!(reason, "too many wrong requests");
                assert!(!failed);
            }
            _ => panic!("expected ShareEnded"),
        }
    }

    #[tokio::test]
    async fn early_exit_reports_stderr_without_the_invite() {
        let s = stub("early", "import sys\nsys.stdin.readline()\nprint('tetron-invite-serve: no default route',file=sys.stderr)\nsys.exit(1)\n");
        let (tx, mut rx) = unbounded_channel();
        tokio::spawn(run("python3", s, "n".into(), "SECRETCODE".into(), "i3".into(), tx, Arc::new(Notify::new())));
        match rx.recv().await.unwrap() {
            ActionResult::ShareEnded { reason, failed, .. } => {
                assert!(failed);
                assert!(reason.contains("no default route"), "{reason}");
                assert!(!reason.contains("SECRETCODE"));
            }
            _ => panic!("expected ShareEnded"),
        }
    }
}
