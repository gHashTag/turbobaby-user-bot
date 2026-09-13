//! Repository contract for the TurboBaby owner-agent definition (issue #23).
//!
//! The README already presents this file as a repository path.  Keep that link
//! resolvable and keep the target visible to Git, otherwise the documentation can
//! pass locally while the linked agent definition is absent from GitHub.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const OWNER_AGENT_PATH: &str = ".claude/agents/turbobaby-bot.md";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn markdown_link_targets(markdown: &str) -> Vec<&str> {
    markdown
        .split("](")
        .skip(1)
        .filter_map(|tail| tail.split_once(')').map(|(target, _)| target))
        .collect()
}

#[test]
fn readme_owner_agent_link_resolves() {
    let root = repository_root();
    let readme_path = root.join("README.md");
    let readme = fs::read_to_string(&readme_path).expect("README.md must be readable");
    let targets = markdown_link_targets(&readme);

    assert!(
        !targets.is_empty(),
        "README link scan found no Markdown targets; suspect the scan, not the repository"
    );

    let owner_links: Vec<_> = targets
        .into_iter()
        .filter(|target| *target == OWNER_AGENT_PATH)
        .collect();
    assert!(
        !owner_links.is_empty(),
        "README.md must link to {OWNER_AGENT_PATH}"
    );

    for target in owner_links {
        assert!(
            root.join(target).is_file(),
            "README.md link {target} does not resolve to a repository file"
        );
    }
}

#[test]
fn owner_agent_path_is_not_ignored() {
    let root = repository_root();
    let agent_path = root.join(OWNER_AGENT_PATH);
    assert!(
        agent_path.is_file(),
        "owner-agent definition is absent: {OWNER_AGENT_PATH}"
    );

    let output = Command::new("git")
        .args([
            "check-ignore",
            "--no-index",
            "--quiet",
            "--",
            OWNER_AGENT_PATH,
        ])
        .current_dir(&root)
        .output()
        .expect("git must be available for the repository tracking gate");

    match output.status.code() {
        Some(1) => {}
        Some(0) => panic!(
            "{OWNER_AGENT_PATH} is ignored, so README links resolve locally but not in a clone"
        ),
        code => panic!(
            "git check-ignore failed with status {code:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}
