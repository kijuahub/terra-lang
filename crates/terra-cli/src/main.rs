//! Terra command line. If this executable carries a bytecode trailer it runs
//! that program; otherwise it is the project manager and compiler.

use std::env;
use std::fs;
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use terra_diagnostics::Lang;
use terra_project::{self as project, Project};

#[derive(Parser)]
#[command(name = "terra", version, about = "Terra compiler and project manager")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create or list projects.
    Project {
        #[command(subcommand)]
        action: ProjectCmd,
    },
    /// Run a project or a single .terra file.
    Run {
        /// `project` or a path to a .terra file.
        what: String,
        /// Registered project name. Used with `terra run project`.
        #[arg(long)]
        name: Option<String>,
        /// Message language: en or ru.
        #[arg(long)]
        lang: Option<String>,
    },
    /// Compile the current project to an executable.
    Build {
        /// host, windows, linux, or all.
        #[arg(long, default_value = "host")]
        target: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Debug a project or a .terra file.
    Debug {
        what: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        lang: Option<String>,
    },
    /// Git helpers for the current project.
    Git {
        #[command(subcommand)]
        action: GitCmd,
    },
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// Write terra.toml, sources, and register the project.
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        package: Option<String>,
        /// `latest` or the installed compiler version.
        #[arg(long)]
        compiler: Option<String>,
        #[arg(long)]
        version: Option<String>,
        /// en (default) or ru.
        #[arg(long)]
        lang: Option<String>,
    },
    /// List registered projects and their absolute paths.
    List,
}

#[derive(Subcommand)]
enum GitCmd {
    Init,
    Status,
    Add { paths: Vec<String> },
    Commit {
        #[arg(short, long)]
        message: String,
    },
    Log,
}

fn main() {
    if let Some(code) = embedded_program() {
        std::process::exit(code);
    }
    let mut argv: Vec<String> = env::args().collect();
    let mut program_args = Vec::new();
    if let Some(split) = argv.iter().position(|a| a == "--") {
        program_args = argv.split_off(split);
        program_args.remove(0);
    }
    let cli = Cli::parse_from(argv);
    match dispatch(cli, &program_args) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn embedded_program() -> Option<i32> {
    let path = env::current_exe().ok()?;
    let bytes = fs::read(&path).ok()?;
    let payload = terra_bytecode::read_embedded(&bytes)?;
    let module = terra_bytecode::decode(&payload).ok()?;
    let lang = terra_runtime::lang_of(&module);
    let args: Vec<String> = env::args().skip(1).collect();
    let outcome = terra_runtime::execute(&module, &args, lang, true);
    Some(outcome.exit)
}

fn dispatch(cli: Cli, program_args: &[String]) -> Result<i32, String> {
    match cli.cmd {
        Cmd::Project { action } => project_cmd(action),
        Cmd::Run { what, name, lang } => {
            let (proj_or_file, lang) = load_target(&what, name.as_deref(), lang.as_deref())?;
            run_target(proj_or_file, lang, program_args)
        }
        Cmd::Build { target, name } => {
            let project = open_project(name.as_deref())?;
            let paths = project::build(&project, &target)?;
            for path in &paths {
                println!("{}", path.display());
            }
            Ok(0)
        }
        Cmd::Debug { what, name, lang } => {
            let (target, lang) = load_target(&what, name.as_deref(), lang.as_deref())?;
            let module = match target {
                Target::Project(project) => project::compile_project(&project, lang)?,
                Target::File(path) => compile_path(&path, lang)?,
            };
            terra_debugger::repl(module, program_args, lang);
            Ok(0)
        }
        Cmd::Git { action } => git_cmd(action),
    }
}

enum Target {
    Project(Project),
    File(PathBuf),
}

fn project_cmd(action: ProjectCmd) -> Result<i32, String> {
    match action {
        ProjectCmd::Init {
            name,
            package,
            compiler,
            version,
            lang,
        } => {
            let dir = env::current_dir().map_err(|e| e.to_string())?;
            let report = project::init(
                &dir,
                name.as_deref(),
                package.as_deref(),
                compiler.as_deref(),
                version.as_deref(),
                lang.as_deref(),
            )?;
            println!(
                "initialized {} ({}) at {}",
                report.project.manifest.name,
                report.project.manifest.package,
                report.project.root.display()
            );
            if let Some(warning) = report.git_warning {
                eprintln!("warning: {warning}");
            }
            Ok(0)
        }
        ProjectCmd::List => {
            let projects = project::list_projects()?;
            if projects.is_empty() {
                println!("no Terra projects registered");
                return Ok(0);
            }
            for project in projects {
                println!("{}\t{}", project.name, project.path.display());
            }
            Ok(0)
        }
    }
}

fn git_cmd(action: GitCmd) -> Result<i32, String> {
    let dir = env::current_dir().map_err(|e| e.to_string())?;
    let project = project::open_cwd(&dir)?;
    let text = match action {
        GitCmd::Init => project::git(&project.root, &["init".into()])?,
        GitCmd::Status => project::git(&project.root, &["status".into()])?,
        GitCmd::Add { paths } => {
            let mut args = vec!["add".into()];
            if paths.is_empty() {
                args.push("-A".into());
            } else {
                args.extend(paths);
            }
            project::git(&project.root, &args)?
        }
        GitCmd::Commit { message } => {
            project::git(&project.root, &["commit".into(), "-m".into(), message])?
        }
        GitCmd::Log => project::git(&project.root, &["log".into(), "--oneline".into()])?,
    };
    print!("{text}");
    if !text.is_empty() && !text.ends_with('\n') {
        println!();
    }
    Ok(0)
}

fn open_project(name: Option<&str>) -> Result<Project, String> {
    if let Some(name) = name {
        project::open_name(name)
    } else {
        let dir = env::current_dir().map_err(|e| e.to_string())?;
        project::open_cwd(&dir)
    }
}

fn load_target(what: &str, name: Option<&str>, lang: Option<&str>) -> Result<(Target, Lang), String> {
    if what == "project" {
        let project = open_project(name)?;
        let lang = parse_lang(lang)?.unwrap_or(project.manifest.lang);
        Ok((Target::Project(project), lang))
    } else {
        let lang = parse_lang(lang)?.unwrap_or(Lang::En);
        Ok((Target::File(PathBuf::from(what)), lang))
    }
}

fn run_target(target: Target, lang: Lang, args: &[String]) -> Result<i32, String> {
    let module = match target {
        Target::Project(project) => project::compile_project(&project, lang)?,
        Target::File(path) => compile_path(&path, lang)?,
    };
    let outcome = terra_runtime::execute(&module, args, lang, true);
    Ok(outcome.exit)
}

fn compile_path(path: &std::path::Path, lang: Lang) -> Result<terra_bytecode::Module, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.display().to_string();
    terra_compiler::compile_file(&name, &source, lang)
        .map(|c| c.module)
        .map_err(|e| e.messages.join("\n"))
}

fn parse_lang(value: Option<&str>) -> Result<Option<Lang>, String> {
    match value {
        None => Ok(None),
        Some(value) => Lang::parse(value)
            .map(Some)
            .ok_or_else(|| format!("unknown language '{value}'. Use en or ru")),
    }
}
