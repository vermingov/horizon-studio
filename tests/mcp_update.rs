// Updating horizon-studio-mcp (net::hstudio_mcp_update) and a running
// server switching to a new binary (mcp::serve).
// The archive and server tests run Unix scripts: elsewhere their helpers go unused.
#![cfg_attr(not(unix), allow(unused))]

use horizon_studio::net::hstudio_mcp_update::{install_archive, installed, newer, note_installed};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("hs-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// A gzipped tar of one file.
fn tar_gz(name: &str, data: &[u8]) -> Vec<u8> {
    let mut tar = vec![0u8; 512];
    tar[..name.len()].copy_from_slice(name.as_bytes());
    tar[100..108].copy_from_slice(b"0000755\0");
    let size = format!("{:011o}\0", data.len());
    tar[124..136].copy_from_slice(size.as_bytes());
    tar[156] = b'0';
    // The header's checksum, counting its own field as spaces.
    tar[148..156].copy_from_slice(b"        ");
    let sum: u32 = tar[..512].iter().map(|b| u32::from(*b)).sum();
    tar[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    tar.extend_from_slice(data);
    tar.resize(tar.len().div_ceil(512) * 512 + 1024, 0);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar).unwrap();
    gz.finish().unwrap()
}

fn zip_of(name: &str, data: &[u8]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut out);
    z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
    z.write_all(data).unwrap();
    z.finish().unwrap();
    out.into_inner()
}

#[test]
fn versions_compare_by_their_numbers() {
    assert!(newer("0.10.0", "0.9.2"));
    assert!(newer("v0.3.0", "0.2.0"));
    assert!(!newer("0.2.0", "0.2.0"));
    assert!(!newer("0.1.9", "0.2.0"));
}

#[test]
fn the_install_is_noted_for_the_game() {
    let dir = temp("note");
    assert_eq!(installed(&dir), None);
    let exe = dir.join("horizon-studio-mcp");
    std::fs::write(&exe, "x").unwrap();
    note_installed(&dir, &exe, "0.2.0");
    assert_eq!(installed(&dir), Some((exe.clone(), "0.2.0".into())));
    std::fs::remove_file(&exe).unwrap();
    assert_eq!(installed(&dir), None, "a server no longer there");
}

#[cfg(unix)]
#[test]
fn a_release_archive_replaces_the_binary_once_checked() {
    let dir = temp("archive");
    let target = dir.join("horizon-studio-mcp");
    std::fs::write(&target, "#!/bin/sh\necho old\n").unwrap();
    let new = b"#!/bin/sh\necho horizon-studio-mcp 9.9.9\n";
    let asset = "horizon-studio-mcp-x86_64-unknown-linux-musl.tar.gz";
    let archive = tar_gz("horizon-studio-mcp", new);
    let sums = format!("{}  {asset}\n{}  other.zip\n", sha256(&archive), "0".repeat(64));
    // A wrong checksum, or none, leaves the old one.
    let bad = format!("{}  {asset}\n", "0".repeat(64));
    assert!(
        install_archive(&target, asset, &archive, &bad)
            .unwrap_err()
            .contains("checksum")
    );
    assert!(
        install_archive(&target, asset, &archive, "")
            .unwrap_err()
            .contains("has no")
    );
    assert!(std::fs::read_to_string(&target).unwrap().contains("old"));
    // The right one goes in, runnable, and nothing is left beside it.
    let version = install_archive(&target, asset, &archive, &sums).unwrap();
    assert_eq!(version, "horizon-studio-mcp 9.9.9");
    assert_eq!(std::fs::read(&target).unwrap(), new);
    assert!(!dir.join("horizon-studio-mcp.new").exists());
    // One that doesn't run is not put in place.
    let broken = tar_gz("horizon-studio-mcp", b"#!/bin/sh\nexit 1\n");
    let sums = format!("{}  {asset}\n", sha256(&broken));
    assert!(
        install_archive(&target, asset, &broken, &sums)
            .unwrap_err()
            .contains("doesn't run")
    );
    assert_eq!(std::fs::read(&target).unwrap(), new);
    // An archive without the server.
    let other = tar_gz("README.md", b"hi");
    let sums = format!("{}  {asset}\n", sha256(&other));
    assert!(
        install_archive(&target, asset, &other, &sums)
            .unwrap_err()
            .contains("has no horizon-studio-mcp")
    );
    // Windows' .zip the same way.
    let zipped = zip_of("horizon-studio-mcp", new);
    let zasset = "horizon-studio-mcp-x86_64-pc-windows-msvc.zip";
    let sums = format!("{}  {zasset}\n", sha256(&zipped));
    assert_eq!(
        install_archive(&target, zasset, &zipped, &sums).unwrap(),
        "horizon-studio-mcp 9.9.9"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The server, running from a copy in `dir`, with its own home.
#[cfg(unix)]
fn start(
    dir: &Path,
) -> (
    std::process::Child,
    std::process::ChildStdin,
    BufReader<std::process::ChildStdout>,
) {
    let exe = dir.join("horizon-studio-mcp");
    let mut child = std::process::Command::new(&exe)
        .env("HOME", dir)
        .env("HORIZON_STUDIO_DOCS_URL", "off")
        .env("HORIZON_STUDIO_LINK", dir.join("link.json"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    (child, stdin, stdout)
}

#[cfg(unix)]
fn read(out: &mut BufReader<std::process::ChildStdout>) -> serde_json::Value {
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line:?}"))
}

#[cfg(unix)]
#[test]
fn a_running_server_switches_to_a_new_binary_at_its_next_request() {
    let dir = temp("reload");
    let exe = dir.join("horizon-studio-mcp");
    std::fs::copy(env!("CARGO_BIN_EXE_horizon-studio-mcp"), &exe).unwrap();
    let (mut child, mut stdin, mut out) = start(&dir);
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","clientInfo":{{"name":"claude-code"}}}}}}"#
    )
    .unwrap();
    let init = read(&mut out);
    assert_eq!(init["result"]["capabilities"]["tools"]["listChanged"], true);
    // It noted where it is installed, for the game's update button.
    assert_eq!(installed(&dir.join(".horizon-studio")).map(|i| i.0), Some(exe.clone()));
    // A new binary takes its place (as an update renames one in).
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let fresh = dir.join("fresh");
    std::fs::copy(env!("CARGO_BIN_EXE_horizon-studio-mcp"), &fresh).unwrap();
    std::fs::rename(&fresh, &exe).unwrap();
    // Two requests at once: both reach the new server.
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":3,"method":"ping"}}"#).unwrap();
    let first = read(&mut out);
    assert_eq!(first["method"], "notifications/tools/list_changed", "{first}");
    let tools = read(&mut out);
    assert_eq!(tools["id"], 2);
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "update_server")
    );
    assert_eq!(read(&mut out)["id"], 3);
    // The same process, still answering.
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":4,"method":"ping"}}"#).unwrap();
    assert_eq!(read(&mut out)["id"], 4);
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(dir);
}
