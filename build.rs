use std::process::Command;

fn main() {
    println!(">>> Building version number...");

    let rustc = std::env::var("RUSTC").unwrap();
    let out = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("failed to run `rustc --version`");
    let rustc_version =
        String::from_utf8(out.stdout).expect("rustc --version returned invalid UTF-8");
    let rustc_version = rustc_version.trim();
    println!("cargo:rustc-env=CARGO_RUSTC_VERSION={}", rustc_version);

    println!("cargo:rerun-if-env-changed=BFFHD_BUILD_TAGGED_RELEASE");
    let tagged_release = option_env!("BFFHD_BUILD_TAGGED_RELEASE") == Some("1");
    let version_string = if tagged_release {
        format!(
            "{version} [{rustc}]",
            version = env!("CARGO_PKG_VERSION"),
            rustc = rustc_version
        )
    } else {
        // Build version number using the current git commit id
        let out = Command::new("git")
            .arg("rev-list")
            .args(["HEAD", "-1"])
            .output()
            .expect("failed to run `git rev-list HEAD -1`");
        let owned_gitrev =
            String::from_utf8(out.stdout).expect("git rev-list output was not valid UTF8");
        let gitrev = owned_gitrev.trim();
        let abbrev = match gitrev.len() {
            0 => "unknown",
            _ => &gitrev[0..9],
        };

        let out = Command::new("git")
            .arg("log")
            .args(["-1", "--format=%as"])
            .output()
            .expect("failed to run `git log -1 --format=\"format:%as\"`");
        let commit_date = String::from_utf8(out.stdout).expect("git log output was not valid UTF8");
        let commit_date = commit_date.trim();

        format!(
            "{version} ({gitrev} {date}) [{rustc}]",
            version = env!("CARGO_PKG_VERSION"),
            gitrev = abbrev,
            date = commit_date,
            rustc = rustc_version
        )
    };
    println!("cargo:rustc-env=BFFHD_VERSION_STRING={}", version_string);
    println!(
        "cargo:rustc-env=BFFHD_RELEASE_STRING=\"BFFH {}\"",
        version_string
    );
}
