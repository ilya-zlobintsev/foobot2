use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=proto/");
    tonic_build::compile_protos("proto/foobot.proto").unwrap();

    println!("cargo:rerun-if-changed=.git/");

    let commit_output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .unwrap();

    let version = String::from_utf8(commit_output.stdout).unwrap();

    println!("cargo:rustc-env=GIT_HASH={}", version);

    println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
}
