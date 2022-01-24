use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/");

    let commit_output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .unwrap();

    let version = String::from_utf8(commit_output.stdout).unwrap();

    let diff_output = Command::new("git")
        .args(&["--no-pager", "diff", "HEAD"])
        .output()
        .unwrap();

    let git_diff = String::from_utf8(diff_output.stdout).unwrap();

    let uncommited_changes = !git_diff.trim().is_empty();

    println!("cargo:rustc-env=GIT_HASH={}", version);
    println!("cargo:rustc-env=GIT_UNCOMMITED_CHANGES={}", uncommited_changes);
}
