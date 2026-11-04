//! Save an invite to a file for `tetron join --invite-file` (TUI-INVITE-001).

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Keep `[A-Za-z0-9._-]`, replace the rest with `_`, so a network name can
/// never escape `dir` or hide in a dot file.
fn safe(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
        .collect();
    t.trim_start_matches('.').to_string()
}

/// Write `code` and a newline to `dir/tetron-invite-<network>-<id>.txt`,
/// mode 0600, never overwriting. Returns the path.
pub fn save(dir: &Path, network: &str, id: &str, code: &str) -> Result<PathBuf, String> {
    if code.trim().is_empty() || code.contains('\n') {
        return Err("save invite: the invite is empty or not one line".into());
    }
    let path = dir.join(format!("tetron-invite-{}-{}.txt", safe(network), safe(id)));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("save invite: {}: {e}", path.display()))?;
    f.write_all(format!("{code}\n").as_bytes())
        .map_err(|e| format!("save invite: write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tetron-tui-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn writes_code_and_newline_mode_0600() {
        let d = tmpdir("save");
        let p = save(&d, "home", "ab12", "CODE").unwrap();
        assert_eq!(p, d.join("tetron-invite-home-ab12.txt"));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "CODE\n");
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn refuses_to_overwrite() {
        let d = tmpdir("exist");
        save(&d, "n", "i", "ONE").unwrap();
        let e = save(&d, "n", "i", "TWO").unwrap_err();
        assert!(e.contains("exists"), "{e}");
        assert_eq!(std::fs::read_to_string(d.join("tetron-invite-n-i.txt")).unwrap(), "ONE\n");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn network_name_can_not_leave_the_dir() {
        let d = tmpdir("safe");
        let p = save(&d, "../../etc/x y", "..", "C").unwrap();
        assert_eq!(p.parent().unwrap(), d);
        assert_eq!(p.file_name().unwrap(), "tetron-invite-_.._etc_x_y-.txt");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rejects_empty_or_multiline_code() {
        let d = tmpdir("bad");
        assert!(save(&d, "n", "i", " ").is_err());
        assert!(save(&d, "n", "i", "a\nb").is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
