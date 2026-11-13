use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn terra() -> Command {
    Command::new(env!("CARGO_BIN_EXE_terra"))
}

fn scratch(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "terra-cli-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    (root, home)
}

fn out(cmd: &mut Command) -> (bool, String, String) {
    let output = cmd.output().expect("spawn terra");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn init_list_run_and_build() {
    let (root, home) = scratch("init");
    let project = root.join("MyApp");
    fs::create_dir_all(&project).unwrap();

    let (ok, stdout, stderr) = out(
        terra()
            .env("TERRA_HOME", &home)
            .current_dir(&project)
            .args([
                "project",
                "init",
                "--name",
                "MyApp",
                "--package",
                "com.example.app",
                "--compiler",
                "latest",
                "--version",
                "1.0.0",
                "--lang",
                "en",
            ]),
    );
    assert!(ok, "{stdout}\n{stderr}");
    assert!(project.join("terra.toml").is_file());
    assert!(project
        .join("src")
        .join("com")
        .join("example")
        .join("app")
        .join("App.terra")
        .is_file());

    let (ok, list, stderr) = out(terra().env("TERRA_HOME", &home).args(["project", "list"]));
    assert!(ok, "{stderr}");
    assert!(list.contains("MyApp"), "{list}");
    assert!(
        list.contains(&project.canonicalize().unwrap().display().to_string())
            || list.contains("MyApp"),
        "{list}"
    );

    let elsewhere = root.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let (ok, stdout, stderr) = out(
        terra()
            .env("TERRA_HOME", &home)
            .current_dir(&elsewhere)
            .args(["run", "project", "--name", "MyApp"]),
    );
    assert!(ok, "{stdout}\n{stderr}");
    assert!(stdout.contains("Hello, Terra"), "{stdout}");

    let (ok, paths, stderr) = out(
        terra()
            .env("TERRA_HOME", &home)
            .current_dir(&project)
            .args(["build"]),
    );
    assert!(ok, "{paths}\n{stderr}");
    let exe = if cfg!(windows) {
        project.join("target").join("MyApp.exe")
    } else {
        project.join("target").join("MyApp")
    };
    assert!(exe.is_file(), "missing {}", exe.display());
    let built = Command::new(&exe).output().expect("run built exe");
    let built_out = String::from_utf8_lossy(&built.stdout);
    assert!(built.status.success(), "{built_out}{}", String::from_utf8_lossy(&built.stderr));
    assert!(built_out.contains("Hello, Terra"), "{built_out}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn run_refuses_uninitialized_directory() {
    let (root, home) = scratch("empty");
    let (ok, stdout, stderr) = out(
        terra()
            .env("TERRA_HOME", &home)
            .current_dir(&root)
            .args(["run", "project"]),
    );
    assert!(!ok, "{stdout}");
    let text = format!("{stdout}{stderr}").to_lowercase();
    assert!(text.contains("terra.toml") || text.contains("not a terra project"), "{text}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unknown_compiler_is_rejected() {
    let (root, home) = scratch("compiler");
    let (ok, _, stderr) = out(
        terra()
            .env("TERRA_HOME", &home)
            .current_dir(&root)
            .args(["project", "init", "--compiler", "9.9.9"]),
    );
    assert!(!ok, "{stderr}");
    assert!(stderr.contains("9.9.9"), "{stderr}");
    let _ = fs::remove_dir_all(&root);
}
