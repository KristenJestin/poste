//! The real binary on a temporary home folder: a project registered in Unity Hub and one
//! found under a root, in the launcher's format.

use serde_json::Value;
use std::fs;
use std::process::Command;

#[test]
fn projects_json() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let nested = home.join("Projects/work/_worktrees/kit");
    fs::create_dir_all(nested.join("ProjectSettings")).unwrap();
    fs::write(
        nested.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 6000.5.4f1\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_poste-unity"))
        .args(["projects", "--json"])
        .env_clear()
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let items = v.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "kit");
    assert_eq!(
        items[0]["subtitle"],
        "6000.5.4f1 · ~/Projects/work/_worktrees/kit"
    );
}

#[test]
fn bad_settings_give_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    fs::create_dir_all(home.join(".config/poste")).unwrap();
    fs::write(home.join(".config/poste/unity.toml"), "depth = 0\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_poste-unity"))
        .args(["projects", "--json"])
        .env_clear()
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["id"], "unity-error");
}
