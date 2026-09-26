//! The layers only depend inwards, checked against the source under `src/`,
//! where each layer is a directory:
//!
//! ```text
//! src/core/         entity · usecase · message    what Pulsline is: no I/O
//! src/interface/    cli (tui · mcp later)          the ways in: a person, an agent, a script
//! src/infra/        spool · index · dispatch       the systems it calls on
//! src/main.rs, lib.rs, config.rs, logging.rs      beside the layers
//! desktop/src/                                     the desktop client: core and infra, never interface
//! ```
//!
//! Inside `core`, `entity` is innermost, then `usecase`, then `message`.
//! `interface` and `infra` depend on `core` and not on each other — except
//! `interface/cli`, whose subcommands each run on their own and assemble the
//! infra they need. The core stays clear of the crates and the parts of
//! `std` that touch files, the network, the terminal, processes, or the
//! environment. See AGENTS.md ("Architecture").

use std::path::{Path, PathBuf};

/// A layer: its directory (or file) under `src/`, the crate modules it may
/// name — `core::entity` allows `crate::core::entity::…` — and the external
/// crates it must not use.
struct Layer {
    path: &'static str,
    allowed: &'static [&'static str],
    forbidden_crates: &'static [&'static str],
}

/// Crates for storage, the terminal, drawing, the network, the runtime, and
/// logging: all outside the core.
const OUTER_CRATES: &[&str] = &[
    "rusqlite",
    "directories",
    "clap",
    "tracing",
    "tracing_subscriber",
    "gpui",
    "ratatui",
    "crossterm",
    "tokio",
    "reqwest",
];

/// The parts of `std` that do I/O or read the machine's state.
const IO_STD: &[&str] = &["std::fs", "std::net", "std::process", "std::env", "std::io"];

const LAYERS: &[Layer] = &[
    Layer {
        path: "core/entity",
        allowed: &["core::entity"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "core/usecase",
        allowed: &["core::entity", "core::usecase"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "core/message.rs",
        allowed: &["core::entity", "core::usecase", "core::message"],
        forbidden_crates: OUTER_CRATES,
    },
    Layer {
        path: "interface/cli",
        allowed: &["core", "interface", "infra", "config", "logging"],
        forbidden_crates: &[],
    },
    Layer {
        path: "infra",
        allowed: &["core", "infra", "config"],
        forbidden_crates: &["clap", "ratatui", "crossterm", "gpui"],
    },
];

/// Every module path a line names through `crate::`, outside comments, with
/// a `{…}` group spread into one path per item: `crate::core::{entity,
/// usecase}` names `core::entity` and `core::usecase`.
fn crate_paths(line: &str) -> Vec<String> {
    paths_after(line, "crate::")
}

/// Every module path a line names after `prefix` (`crate::`, `pulsline::`).
fn paths_after(line: &str, prefix: &str) -> Vec<String> {
    let code = line.split("//").next().unwrap_or_default();
    let is_path = |c: char| c.is_alphanumeric() || c == '_' || c == ':';
    let mut paths = Vec::new();
    for (at, _) in code.match_indices(prefix) {
        let before = code[..at].chars().last();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == ':') {
            continue;
        }
        let rest = &code[at + prefix.len()..];
        let end = rest.find(|c: char| !is_path(c)).unwrap_or(rest.len());
        let path = &rest[..end];
        if rest[end..].starts_with('{') && (path.is_empty() || path.ends_with("::")) {
            let group = &rest[end + 1..];
            let group = &group[..group.find('}').unwrap_or(group.len())];
            for item in group.split(',') {
                let item = item.trim();
                let item_end = item.find(|c: char| !is_path(c)).unwrap_or(item.len());
                paths.push(format!("{path}{}", &item[..item_end]));
            }
        } else {
            paths.push(path.trim_end_matches(':').to_string());
        }
    }
    paths
}

/// Whether `path` is `module` or inside it.
fn within(path: &str, module: &str) -> bool {
    path == module
        || path
            .strip_prefix(module)
            .is_some_and(|rest| rest.starts_with("::"))
}

/// Whether a line uses the external crate `name`, outside comments.
fn uses_crate(line: &str, name: &str) -> bool {
    let code = line.split("//").next().unwrap_or_default();
    let path = format!("{name}::");
    code.match_indices(&path).any(|(at, _)| {
        // `rusqlite::` on its own, not `crate::rusqlite_thing::` or `my_tokio::`.
        code[..at]
            .chars()
            .last()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == ':'))
    })
}

/// Whether a line names `std_path` (`std::fs`), directly or in a `use std::{…}`
/// group, outside comments.
fn uses_std(line: &str, std_path: &str) -> bool {
    let code = line.split("//").next().unwrap_or_default();
    let module = std_path.trim_start_matches("std::");
    paths_after(code, "std::")
        .iter()
        .any(|path| within(path, module))
}

fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            rust_files(&entry.path(), out);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        out.push(path.to_path_buf());
    }
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn src() -> PathBuf {
    root().join("src")
}

/// Each line of every file under `dir`, with where it is.
fn lines_in(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_files(dir, &mut files);
    assert!(!files.is_empty(), "no sources under {}", dir.display());
    let mut out = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let shown = file
            .strip_prefix(root())
            .unwrap_or(&file)
            .display()
            .to_string();
        for (n, line) in text.lines().enumerate() {
            out.push((format!("{shown}:{}", n + 1), line.to_string()));
        }
    }
    out
}

fn lines_under(path: &str) -> Vec<(String, String)> {
    lines_in(&src().join(path))
}

/// Each layer names only what it is allowed to, and uses none of the crates
/// it is kept clear of.
#[test]
fn every_layer_depends_only_inwards() {
    let mut violations = Vec::new();
    for layer in LAYERS {
        for (at, line) in lines_under(layer.path) {
            for path in crate_paths(&line) {
                if !layer.allowed.iter().any(|module| within(&path, module)) {
                    violations.push(format!("{at}: {} names crate::{path}", layer.path));
                }
            }
            for name in layer.forbidden_crates {
                if uses_crate(&line, name) {
                    violations.push(format!("{at}: {} uses the {name} crate", layer.path));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "dependencies pointing the wrong way:\n{}",
        violations.join("\n")
    );
}

/// The core does no I/O: it names no part of `std` that touches files, the
/// network, processes, or the environment.
#[test]
fn the_core_does_no_io() {
    let io: Vec<String> = lines_under("core")
        .into_iter()
        .filter(|(_, line)| IO_STD.iter().any(|p| uses_std(line, p)))
        .map(|(at, line)| format!("{at}: {}", line.trim()))
        .collect();
    assert!(io.is_empty(), "I/O in the core:\n{}", io.join("\n"));
}

/// The desktop client builds on the core and the infra, and never on the
/// interface: it is an interface of its own.
#[test]
fn the_desktop_client_uses_the_core_and_infra_only() {
    let allowed = ["core", "infra", "config", "logging", "VERSION"];
    let wrong: Vec<String> = lines_in(&root().join("desktop/src"))
        .into_iter()
        .flat_map(|(at, line)| {
            paths_after(&line, "pulsline::")
                .into_iter()
                .filter(|path| !allowed.iter().any(|m| within(path, m)))
                .map(move |path| format!("{at}: names pulsline::{path}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The core is always `crate::core`: a bare `core::` is Rust's own crate,
/// and reads as if it were ours.
#[test]
fn the_core_is_always_named_through_crate() {
    let bare: Vec<String> = lines_in(&src())
        .into_iter()
        .filter(|(_, line)| uses_crate(line, "core"))
        .map(|(at, line)| format!("{at}: {}", line.trim()))
        .collect();
    assert!(bare.is_empty(), "bare core:: paths:\n{}", bare.join("\n"));
}

/// The layers table covers every directory under `src/`, so a new one is
/// placed on purpose.
#[test]
fn every_source_directory_is_in_a_layer() {
    let mut dirs = Vec::new();
    for top in ["core", "interface", "infra"] {
        for entry in std::fs::read_dir(src().join(top)).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "mod.rs" {
                dirs.push(format!("{top}/{name}"));
            }
        }
    }
    let unplaced: Vec<&String> = dirs
        .iter()
        .filter(|dir| {
            !LAYERS
                .iter()
                .any(|layer| dir.as_str() == layer.path || within_dir(dir, layer.path))
        })
        .collect();
    assert!(unplaced.is_empty(), "not in any layer: {unplaced:?}");
    for entry in std::fs::read_dir(src()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            ["core", "interface", "infra"].contains(&name.as_str()) || name.ends_with(".rs"),
            "src/{name} is a directory outside the layers"
        );
    }
}

/// Whether `dir` lies inside the layer directory `layer`.
fn within_dir(dir: &str, layer: &str) -> bool {
    dir.strip_prefix(layer)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// The checker itself: it sees module paths, groups, and crate paths, and
/// not ones in comments.
#[test]
fn the_checker_reads_paths_and_skips_comments() {
    assert_eq!(
        crate_paths("use crate::core::entity::Event; // crate::infra"),
        ["core::entity::Event"]
    );
    assert_eq!(
        crate_paths("use crate::core::{entity, usecase::Request};"),
        ["core::entity", "core::usecase::Request"]
    );
    assert!(within("core::entity::Event", "core::entity"));
    assert!(!within("core::entityish", "core::entity"));
    assert!(uses_crate("use rusqlite::Connection;", "rusqlite"));
    assert!(!uses_crate("// rusqlite::Connection", "rusqlite"));
    assert!(!uses_crate("use crate::my_tokio::x;", "tokio"));
    assert!(!uses_crate("use crate::core::entity;", "core"));
    assert!(uses_std("use std::fs::File;", "std::fs"));
    assert!(uses_std("use std::{fmt, fs};", "std::fs"));
    assert!(!uses_std("use std::fmt;", "std::fs"));
    assert!(!uses_std("use std::fsx;", "std::fs"));
    assert_eq!(paths_after("pulsline::core::X", "pulsline::"), ["core::X"]);
}
