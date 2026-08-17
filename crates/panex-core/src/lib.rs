pub mod config;

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

pub fn read_directory(path: &str) -> Result<Vec<FileEntry>, String> {
    let dir_path = Path::new(path);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let mut entries = Vec::new();

    let read_dir = fs::read_dir(dir_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata: {}", e))?;

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            size: disk_size(&metadata),
            modified,
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

pub fn rename_entry(path: &str, new_name: &str) -> Result<(), String> {
    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let parent = source
        .parent()
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let dest = parent.join(new_name);

    if dest.exists() {
        return Err(format!("A file named '{}' already exists", new_name));
    }

    fs::rename(&source, &dest).map_err(|e| format!("Failed to rename: {}", e))
}

pub fn open_entry(path: &str) -> Result<(), String> {
    open_entry_with_app(path, None)
}

/// Open a file with a specific application, or the system default if None.
pub fn open_entry_with_app(path: &str, app: Option<&str>) -> Result<(), String> {
    let target = std::path::PathBuf::from(path);
    if !target.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if let Some(app_name) = app {
            cmd.args(["-a", app_name]);
        }
        cmd.arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(app_name) = app {
            std::process::Command::new(app_name)
                .arg(path)
                .spawn()
                .map_err(|e| format!("Failed to open with {}: {}", app_name, e))?;
        } else {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", path])
                .spawn()
                .map_err(|e| format!("Failed to open: {}", e))?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(app_name) = app {
            std::process::Command::new(app_name)
                .arg(path)
                .spawn()
                .map_err(|e| format!("Failed to open with {}: {}", app_name, e))?;
        } else {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| format!("Failed to open: {}", e))?;
        }
    }

    Ok(())
}

/// Get the file extension (without dot) from a path.
pub fn get_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
}

pub fn delete_entry(path: &str, permanent: bool) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    if permanent {
        if target.is_dir() {
            fs::remove_dir_all(&target).map_err(|e| format!("Failed to delete: {}", e))
        } else {
            fs::remove_file(&target).map_err(|e| format!("Failed to delete: {}", e))
        }
    } else {
        {
            #[cfg(target_os = "macos")]
            {
                use trash::macos::{DeleteMethod, TrashContextExtMacos};
                use trash::TrashContext;
                let mut ctx = TrashContext::default();
                ctx.set_delete_method(DeleteMethod::NsFileManager);
                ctx.delete(&target).map_err(|e| format!("Failed to move to trash: {}", e))
            }
            #[cfg(not(target_os = "macos"))]
            {
                trash::delete(&target).map_err(|e| format!("Failed to move to trash: {}", e))
            }
        }
    }
}

pub fn copy_entry(source: &str, dest_dir: &str) -> Result<String, String> {
    let src = PathBuf::from(source);
    if !src.exists() {
        return Err(format!("Source does not exist: {}", source));
    }
    let dest = PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err(format!("Destination is not a directory: {}", dest_dir));
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| "Cannot determine file name".to_string())?;
    let dest_path = dest.join(file_name);

    if src.is_dir() {
        copy_dir_recursive(&src, &dest_path)?;
    } else {
        fs::copy(&src, &dest_path).map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    Ok(dest_path.to_string_lossy().to_string())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Failed to create directory: {}", e))?;

    let entries =
        fs::read_dir(src).map_err(|e| format!("Failed to read source directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_dest = dest.join(entry.file_name());

        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &entry_dest)?;
        } else {
            fs::copy(entry.path(), &entry_dest)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}

pub fn calculate_directory_size(path: &str) -> Result<u64, String> {
    let dir_path = Path::new(path);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    fn walk(dir: &Path) -> u64 {
        let mut total: u64 = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        total += walk(&entry.path());
                    } else {
                        total += disk_size(&meta);
                    }
                }
            }
        }
        total
    }

    Ok(walk(dir_path))
}

/// Returns actual disk usage (blocks * 512) on Unix, logical size on Windows.
#[cfg(unix)]
fn disk_size(meta: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn disk_size(meta: &fs::Metadata) -> u64 {
    meta.len()
}

pub fn create_file(dir: &str, name: &str) -> Result<(), String> {
    let path = Path::new(dir).join(name);
    if path.exists() {
        return Err(format!("A file named '{}' already exists", name));
    }
    fs::File::create(&path).map_err(|e| format!("Failed to create file: {}", e))?;
    Ok(())
}

pub fn create_folder(dir: &str, name: &str) -> Result<(), String> {
    let path = Path::new(dir).join(name);
    if path.exists() {
        return Err(format!("A folder named '{}' already exists", name));
    }
    fs::create_dir(&path).map_err(|e| format!("Failed to create folder: {}", e))?;
    Ok(())
}

pub fn open_in_terminal(path: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        // Prefer iTerm2 if installed, fall back to Terminal.app
        let app = if Path::new("/Applications/iTerm.app").exists() {
            "iTerm"
        } else {
            "Terminal"
        };
        std::process::Command::new("open")
            .args(["-a", app, path])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &format!("cd /d {}", cmd_quote(path))])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try common terminal emulators in order
        let terminals = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"];
        let mut launched = false;
        for term in &terminals {
            let result = if *term == "gnome-terminal" {
                std::process::Command::new(term)
                    .arg("--working-directory")
                    .arg(path)
                    .spawn()
            } else {
                std::process::Command::new(term)
                    .current_dir(path)
                    .spawn()
            };
            if result.is_ok() {
                launched = true;
                break;
            }
        }
        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}

/// Open a terminal running a specific command (e.g., "hx /path/to/file").
pub fn open_in_terminal_with_command(command: &str, args: &[&str]) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut full_cmd = build_shell_command(command, args);
        // Two quoting layers stack here: the shell that iTerm/Terminal feeds
        // the line to, handled above, and the AppleScript string literal the
        // line is embedded in, handled now. Escaping first would bury the
        // AppleScript backslashes inside the shell quotes, where they are
        // literal characters rather than escapes.
        full_cmd = applescript_escape(&full_cmd);

        let app = if Path::new("/Applications/iTerm.app").exists() {
            "iTerm"
        } else {
            "Terminal"
        };

        if app == "iTerm" {
            // Use AppleScript to open a new iTerm tab with the command
            let script = format!(
                r#"tell application "iTerm"
                    activate
                    tell current window
                        create tab with default profile
                        tell current session
                            write text "{}"
                        end tell
                    end tell
                end tell"#,
                full_cmd
            );
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn()
                .map_err(|e| format!("Failed to open iTerm: {}", e))?;
        } else {
            // Terminal.app via AppleScript
            let script = format!(
                r#"tell application "Terminal"
                    activate
                    do script "{}"
                end tell"#,
                full_cmd
            );
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn()
                .map_err(|e| format!("Failed to open Terminal: {}", e))?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let mut full_cmd = command.to_string();
        for arg in args {
            full_cmd.push(' ');
            full_cmd.push_str(&cmd_quote(arg));
        }
        std::process::Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &full_cmd])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let full_cmd = build_shell_command(command, args);
        let terminals = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"];
        let mut launched = false;
        for term in &terminals {
            // Every one of these passes the words after the separator to
            // execvp, so `sh` has to be its own argument — folding it into one
            // string asks the terminal to exec a program named "sh -c ...".
            let result = if *term == "gnome-terminal" {
                std::process::Command::new(term)
                    .args(["--", "sh", "-c", &full_cmd])
                    .spawn()
            } else {
                std::process::Command::new(term)
                    .args(["-e", "sh", "-c", &full_cmd])
                    .spawn()
            };
            if result.is_ok() {
                launched = true;
                break;
            }
        }
        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}

/// Quote one argument for a POSIX shell. Single quotes protect everything
/// except a single quote itself, which has to be closed, escaped, and reopened.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Join a command and its arguments into one line for a POSIX shell.
///
/// `command` comes from the user's config (`[open.tui]`) and is passed through
/// as a shell fragment, so an entry like `"nvim -R"` works as written. The
/// arguments are paths we supply, so those are quoted.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn build_shell_command(command: &str, args: &[&str]) -> String {
    let mut line = command.to_string();
    for arg in args {
        line.push(' ');
        line.push_str(&shell_quote(arg));
    }
    line
}

/// Make a finished command line safe inside an AppleScript string literal.
/// This is the outer of the two quoting layers, not a substitute for the inner
/// one — it says nothing about how a shell will later split the line.
#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Quote one argument for `cmd.exe`. Windows filenames cannot contain `"`, so
/// wrapping is enough; the character is stripped rather than trusted.
#[cfg(target_os = "windows")]
fn cmd_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', ""))
}

pub fn move_entry(source: &str, dest_dir: &str) -> Result<String, String> {
    let src = PathBuf::from(source);
    if !src.exists() {
        return Err(format!("Source does not exist: {}", source));
    }
    let dest = PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err(format!("Destination is not a directory: {}", dest_dir));
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| "Cannot determine file name".to_string())?;
    let dest_path = dest.join(file_name);

    // Try fast rename first (works on same volume)
    match fs::rename(&src, &dest_path) {
        Ok(()) => Ok(dest_path.to_string_lossy().to_string()),
        Err(_) => {
            // Cross-volume: copy then delete
            copy_entry(source, dest_dir)?;
            if src.is_dir() {
                fs::remove_dir_all(&src)
                    .map_err(|e| format!("Copied but failed to remove source: {}", e))?;
            } else {
                fs::remove_file(&src)
                    .map_err(|e| format!("Copied but failed to remove source: {}", e))?;
            }
            Ok(dest_path.to_string_lossy().to_string())
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;

    /// The bug in #1: a path with spaces reached the target program as many
    /// arguments, because nothing ever quoted it for the shell.
    #[test]
    fn quotes_paths_containing_spaces() {
        let line = build_shell_command("mdview", &["/Users/me/A stochastic delay model.md"]);
        assert_eq!(line, "mdview '/Users/me/A stochastic delay model.md'");
    }

    /// An apostrophe has to close the quoting, escape itself, and reopen, or
    /// the line ends mid-string and the shell waits for a terminator.
    #[test]
    fn quotes_paths_containing_apostrophes() {
        assert_eq!(shell_quote("Ivan's notes.md"), r"'Ivan'\''s notes.md'");
    }

    /// Inside single quotes a backslash is a literal, so paths keep theirs.
    #[test]
    fn leaves_backslashes_alone_inside_shell_quotes() {
        assert_eq!(shell_quote(r"a\b"), r"'a\b'");
    }

    #[test]
    fn quotes_every_argument_separately() {
        let line = build_shell_command("diff", &["/tmp/a b", "/tmp/c d"]);
        assert_eq!(line, "diff '/tmp/a b' '/tmp/c d'");
    }

    /// The config value is the user's own shell fragment, so flags survive.
    #[test]
    fn passes_the_command_through_as_a_shell_fragment() {
        let line = build_shell_command("nvim -R", &["/tmp/a b.md"]);
        assert_eq!(line, "nvim -R '/tmp/a b.md'");
    }

    /// The assertions above say the line looks right. This one says a real
    /// shell agrees, which is the property the bug was actually about: with
    /// `printf '[%s]'`, one argument prints once and a split path prints once
    /// per fragment.
    fn one_argument_survives_a_real_shell(path: &str) {
        let line = build_shell_command("printf '[%s]'", &[path]);
        let out = std::process::Command::new("sh")
            .args(["-c", &line])
            .output()
            .expect("failed to run sh");
        assert_eq!(String::from_utf8_lossy(&out.stdout), format!("[{}]", path));
    }

    #[test]
    fn spaced_path_reaches_the_program_as_one_argument() {
        one_argument_survives_a_real_shell("/Users/me/A stochastic delay model.md");
    }

    #[test]
    fn apostrophed_path_reaches_the_program_as_one_argument() {
        one_argument_survives_a_real_shell("/Users/me/Ivan's notes.md");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn escapes_applescript_metacharacters() {
        assert_eq!(applescript_escape(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(applescript_escape(r"a\b"), r"a\\b");
    }

    /// Order matters: the shell quotes go on first, then the whole line is
    /// escaped once for AppleScript. Backslashes in the path survive both.
    #[cfg(target_os = "macos")]
    #[test]
    fn layers_shell_quoting_inside_applescript_escaping() {
        let line = build_shell_command("hx", &[r"/tmp/a b\c"]);
        assert_eq!(applescript_escape(&line), r"hx '/tmp/a b\\c'");
    }

    /// The outer layer against the real AppleScript parser rather than our
    /// idea of it: what osascript hands back has to be the shell line we
    /// built, character for character, or the shell sees something else.
    #[cfg(target_os = "macos")]
    fn survives_applescript(command: &str, path: &str) {
        let line = build_shell_command(command, &[path]);
        let script = format!("return \"{}\"", applescript_escape(&line));
        let out = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .expect("failed to run osascript");
        assert!(
            out.status.success(),
            "osascript rejected {script:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim_end(), line);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn escaped_line_round_trips_through_applescript() {
        survives_applescript("mdview", "/Users/me/A stochastic delay model.md");
        survives_applescript("hx", "/tmp/Ivan's notes.md");
        survives_applescript("hx", r"/tmp/a b\c");
        survives_applescript("hx", "/tmp/a\"b");
    }
}
