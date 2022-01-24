use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/");

    let commit_output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .unwrap();

    let version = String::from_utf8(commit_output.stdout).unwrap();

    println!("cargo:rustc-env=GIT_HASH={}", version);
}
