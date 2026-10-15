//! Desktop hand-offs: showing a file in the default file manager, through the D-Bus interface
//! `org.freedesktop.FileManager1.ShowItems` (Nautilus, Flea, Strata… all implement it: the
//! one owning the name on the bus, or the one D-Bus activates; none is named here), and
//! copying a file to the clipboard as a file (`text/uri-list`).

use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// `file://` plus the path, every byte outside the safe characters encoded as `%XX`.
pub fn file_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for &b in path.as_os_str().as_encoded_bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn show_item(path: &Path) -> Result<()> {
    let conn = zbus::blocking::Connection::session().context("D-Bus session bus")?;
    conn.call_method(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowItems",
        &(vec![file_uri(path)], ""),
    )
    .context("org.freedesktop.FileManager1.ShowItems")?;
    Ok(())
}

/// Puts the file itself on the clipboard (`wl-copy --type text/uri-list`): file managers,
/// browsers and chat applications paste it as a file.
pub fn copy_file(path: &Path) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .args(["--type", "text/uri-list"])
        .stdin(Stdio::piped())
        .spawn()
        .context("wl-copy")?;
    child
        .stdin
        .take()
        .context("wl-copy input")?
        .write_all(format!("{}\r\n", file_uri(path)).as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        bail!("wl-copy: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_uri() {
        assert_eq!(
            file_uri(Path::new("/h/My docs/été#1.txt")),
            "file:///h/My%20docs/%C3%A9t%C3%A9%231.txt"
        );
        assert_eq!(file_uri(Path::new("/a/b-c_d.e~")), "file:///a/b-c_d.e~");
    }
}
