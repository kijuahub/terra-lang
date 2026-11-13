//! Project layout, global registry, build targets, and git.
//!
//! `terra project init` writes `terra.toml` into the current directory and
//! records the absolute path. `terra run project` uses that file and does not
//! walk parent directories. `--name` looks the project up in the registry.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use terra_bytecode::{append_payload, encode};
use terra_compiler::{compile, Unit};
use terra_diagnostics::Lang;

pub fn compiler_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub name: String,
    pub package: String,
    pub version: String,
    pub compiler: String,
    pub lang: Lang,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Registry {
    projects: Vec<RegEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RegEntry {
    name: String,
    path: String,
    package: String,
    version: String,
}

#[derive(Clone, Debug)]
pub struct Registered {
    pub name: String,
    pub path: PathBuf,
    pub package: String,
    pub version: String,
}

pub struct InitReport {
    pub project: Project,
    pub git_warning: Option<String>,
}

#[derive(Deserialize)]
struct RawFile {
    package: RawPackage,
    #[serde(default)]
    tool: RawTool,
}

#[derive(Deserialize)]
struct RawPackage {
    name: String,
    package: String,
    version: String,
    compiler: String,
}

#[derive(Deserialize, Default)]
struct RawTool {
    #[serde(default)]
    lang: Option<String>,
}

pub fn init(
    dir: &Path,
    name: Option<&str>,
    package: Option<&str>,
    compiler: Option<&str>,
    version: Option<&str>,
    lang: Option<&str>,
) -> Result<InitReport, String> {
    let dir = abs_dir(dir)?;
    if dir.join("terra.toml").is_file() {
        return Err(format!(
            "already a Terra project: {}",
            dir.join("terra.toml").display()
        ));
    }
    let name = name
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "app".into())
        });
    let package = match package {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => format!("com.example.{}", package_segment(&name)),
    };
    let version = match version {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => "0.1.0".into(),
    };
    let compiler = resolve_compiler(compiler.unwrap_or("latest"))?;
    let lang = match lang {
        None | Some("") => Lang::En,
        Some(value) => Lang::parse(value).ok_or_else(|| {
            format!("unknown language '{value}'. Use en or ru")
        })?,
    };
    let manifest = Manifest {
        name: name.clone(),
        package: package.clone(),
        version: version.clone(),
        compiler,
        lang,
    };
    write_scaffold(&dir, &manifest)?;
    register(&dir, &manifest)?;
    let git_warning = git_init(&dir);
    Ok(InitReport {
        project: Project { root: dir, manifest },
        git_warning,
    })
}

pub fn list_projects() -> Result<Vec<Registered>, String> {
    Ok(load_registry()?
        .projects
        .into_iter()
        .map(|e| Registered {
            name: e.name,
            path: PathBuf::from(e.path),
            package: e.package,
            version: e.version,
        })
        .collect())
}

pub fn open_cwd(dir: &Path) -> Result<Project, String> {
    let dir = abs_dir(dir)?;
    let manifest_path = dir.join("terra.toml");
    if !manifest_path.is_file() {
        return Err(missing_project(dir.as_path()));
    }
    let manifest = read_manifest(&manifest_path)?;
    Ok(Project { root: dir, manifest })
}

pub fn open_name(name: &str) -> Result<Project, String> {
    let matches: Vec<_> = load_registry()?
        .projects
        .into_iter()
        .filter(|e| e.name == name)
        .collect();
    match matches.len() {
        0 => Err(format!(
            "no registered project named '{name}'. Run `terra project list`"
        )),
        1 => {
            let path = PathBuf::from(&matches[0].path);
            if !path.join("terra.toml").is_file() {
                return Err(format!(
                    "registered project '{name}' is missing at {}",
                    path.display()
                ));
            }
            open_cwd(&path)
        }
        _ => Err(format!(
            "project name '{name}' is registered more than once. Use the project directory"
        )),
    }
}

pub fn compile_project(project: &Project, lang: Lang) -> Result<terra_bytecode::Module, String> {
    let mut units = Vec::new();
    collect_units(&project.root.join("src"), &project.root.join("src"), &mut units)?;
    collect_units(&project.root.join("lib"), &project.root.join("lib"), &mut units)?;
    if units.is_empty() {
        return Err(format!(
            "no .terra sources under {}",
            project.root.join("src").display()
        ));
    }
    compile(&units, lang)
        .map(|c| c.module)
        .map_err(|e| e.messages.join("\n"))
}

pub fn build(project: &Project, target: &str) -> Result<Vec<PathBuf>, String> {
    let module = compile_project(project, project.manifest.lang)?;
    let encoded = encode(&module);
    let out_dir = project.root.join("target");
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let tbc = out_dir.join(format!("{}.tbc", project.manifest.name));
    fs::write(&tbc, &encoded).map_err(|e| e.to_string())?;

    let targets = expand_targets(target)?;
    let mut built = vec![tbc];
    let mut errors = Vec::new();
    for target in targets {
        match link_target(project, &encoded, &out_dir, target) {
            Ok(path) => built.push(path),
            Err(err) => errors.push(err),
        }
    }
    if errors.is_empty() {
        Ok(built)
    } else {
        Err(errors.join("\n"))
    }
}

pub fn git(dir: &Path, args: &[String]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    let output = cmd.output().map_err(|e| format!("git is not available: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let err = String::from_utf8_lossy(&output.stderr);
    if !err.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if output.status.success() {
        Ok(text)
    } else {
        Err(if text.trim().is_empty() {
            format!("git exited with {}", output.status)
        } else {
            text
        })
    }
}

fn link_target(project: &Project, encoded: &[u8], out_dir: &Path, target: &str) -> Result<PathBuf, String> {
    let launcher = launcher_bytes(target)?;
    let bundled = append_payload(launcher, encoded);
    let path = out_dir.join(exe_name(&project.manifest.name, target));
    fs::write(&path, bundled).map_err(|e| e.to_string())?;
    Ok(path)
}

fn launcher_bytes(target: &str) -> Result<Vec<u8>, String> {
    let host_ok = target == "host"
        || (target == "windows" && cfg!(windows))
        || (target == "linux" && cfg!(unix));
    if host_ok {
        let path = std::env::current_exe().map_err(|e| e.to_string())?;
        return fs::read(&path).map_err(|e| format!("cannot read launcher {}: {e}", path.display()));
    }
    let key = if target == "linux" {
        "TERRA_LINUX_LAUNCHER"
    } else {
        "TERRA_WINDOWS_LAUNCHER"
    };
    if let Ok(path) = std::env::var(key) {
        return fs::read(&path).map_err(|e| format!("cannot read {key}={path}: {e}"));
    }
    Err(format!(
        "no {target} launcher. Set {key} to a Terra executable built for that platform. The bytecode (.tbc) was still written"
    ))
}

fn expand_targets(target: &str) -> Result<Vec<&str>, String> {
    match target {
        "host" => Ok(vec!["host"]),
        "windows" => Ok(vec!["windows"]),
        "linux" => Ok(vec!["linux"]),
        "all" => Ok(vec!["windows", "linux"]),
        other => Err(format!(
            "unknown build target '{other}'. Use host, windows, linux, or all"
        )),
    }
}

fn exe_name(name: &str, target: &str) -> String {
    let windows = target == "windows" || (target == "host" && cfg!(windows));
    if windows {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn collect_units(root: &Path, dir: &Path, units: &mut Vec<Unit>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_units(root, &path, units)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("terra") {
            continue;
        }
        let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let package = path
            .parent()
            .and_then(|p| p.strip_prefix(root).ok())
            .map(|rel| {
                rel.components()
                    .filter_map(|c| c.as_os_str().to_str())
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .filter(|p| !p.is_empty());
        units.push(Unit {
            name: path.display().to_string(),
            source,
            package,
        });
    }
    Ok(())
}

fn write_scaffold(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let pkg_dir = dir.join("src").join(manifest.package.replace('.', "/"));
    fs::create_dir_all(&pkg_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("lib")).map_err(|e| e.to_string())?;
    let lang = match manifest.lang {
        Lang::Ru => "ru",
        Lang::En => "en",
    };
    let toml = format!(
        "[package]\nname = \"{}\"\npackage = \"{}\"\nversion = \"{}\"\ncompiler = \"{}\"\n\n[tool]\nlang = \"{lang}\"\n",
        manifest.name, manifest.package, manifest.version, manifest.compiler
    );
    fs::write(dir.join("terra.toml"), toml).map_err(|e| e.to_string())?;
    let app = format!(
        "package {};\n\nfunc main(args: string[]) {{\n    println(\"Hello, Terra\");\n}}\n",
        manifest.package
    );
    fs::write(pkg_dir.join("App.terra"), app).map_err(|e| e.to_string())?;
    fs::write(dir.join("lib").join(".gitkeep"), "").map_err(|e| e.to_string())?;
    fs::write(
        dir.join(".gitignore"),
        "/target\n*.tbc\n*.exe\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        dir.join("README.md"),
        format!("# {}\n\nPackage `{}`.\n\n```bash\nterra run project\nterra build\n```\n", manifest.name, manifest.package),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let raw: RawFile = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let lang = match raw.tool.lang.as_deref() {
        None | Some("") => Lang::En,
        Some(value) => Lang::parse(value).ok_or_else(|| format!("unknown language '{value}' in {}", path.display()))?,
    };
    Ok(Manifest {
        name: raw.package.name,
        package: raw.package.package,
        version: raw.package.version,
        compiler: raw.package.compiler,
        lang,
    })
}

fn register(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let mut registry = load_registry()?;
    if let Some(other) = registry.projects.iter().find(|e| e.name == manifest.name && e.path != dir.display().to_string()) {
        return Err(format!(
            "project '{}' is already registered at {}",
            manifest.name, other.path
        ));
    }
    registry.projects.retain(|e| e.path != dir.display().to_string());
    registry.projects.push(RegEntry {
        name: manifest.name.clone(),
        path: dir.display().to_string(),
        package: manifest.package.clone(),
        version: manifest.version.clone(),
    });
    save_registry(&registry)
}

fn load_registry() -> Result<Registry, String> {
    let path = registry_path()?;
    if !path.is_file() {
        return Ok(Registry { projects: Vec::new() });
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn save_registry(registry: &Registry) -> Result<(), String> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

fn registry_path() -> Result<PathBuf, String> {
    if let Ok(home) = std::env::var("TERRA_HOME") {
        if !home.is_empty() {
            return Ok(PathBuf::from(home).join("registry.json"));
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return Ok(PathBuf::from(appdata).join("terra").join("registry.json"));
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot locate a home directory for the Terra registry".to_string())?;
    Ok(PathBuf::from(home).join(".terra").join("registry.json"))
}

fn resolve_compiler(value: &str) -> Result<String, String> {
    let current = compiler_version();
    if value == "latest" || value == current {
        Ok(current.into())
    } else {
        Err(format!(
            "compiler '{value}' is not installed. This Terra is {current}. Use --compiler latest or {current}"
        ))
    }
}

fn package_segment(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out = "app".into();
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'n');
    }
    out
}

fn abs_dir(dir: &Path) -> Result<PathBuf, String> {
    if dir.as_os_str().is_empty() {
        return Err("empty project directory".into());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))
}

fn git_init(dir: &Path) -> Option<String> {
    match Command::new("git").arg("init").current_dir(dir).output() {
        Ok(out) if out.status.success() => None,
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            let text = err.trim();
            Some(if text.is_empty() {
                "git init failed".into()
            } else {
                text.to_string()
            })
        }
        Err(e) => Some(format!("git is not available ({e}). The project was still created")),
    }
}

fn missing_project(dir: &Path) -> String {
    format!(
        "this directory is not a Terra project (terra.toml not found in {}). Run `terra project init`",
        dir.display()
    )
}
