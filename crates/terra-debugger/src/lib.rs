//! Source debugger on top of the virtual machine.
//!
//! Commands: `b <file>:<line>`, `run`/`c`, `next`/`n`, `step`/`s`, `out`/`o`,
//! `vars`/`v`, `p <name>`, `bt`, `q`, `help`. `$entry` and line 0 are not stops.

use terra_bytecode::Module;
use terra_diagnostics::{rt, Lang, RtMsg};
use terra_runtime::Vm;

const STEP_LIMIT: u32 = 1_000_000;

pub struct Debugger {
    vm: Vm,
    lang: Lang,
    breaks: Vec<(String, u32)>,
    transcript: String,
}

struct Site {
    func: String,
    file: String,
    line: u32,
    user: bool,
}

enum Mode {
    Continue,
    Into,
    Over,
    Out,
}

impl Debugger {
    pub fn new(module: Module, args: &[String], lang: Lang) -> Self {
        Self {
            vm: Vm::new(module, args, lang, false),
            lang,
            breaks: Vec::new(),
            transcript: String::new(),
        }
    }

    pub fn transcript(&self) -> &str {
        &self.transcript
    }

    pub fn stdout(&self) -> &str {
        self.vm.stdout()
    }

    /// `None` means the session ended (`q`).
    pub fn command(&mut self, input: &str) -> Option<String> {
        let input = input.trim();
        if input.is_empty() {
            return Some(String::new());
        }
        let (cmd, rest) = split_cmd(input);
        let text = match cmd {
            "help" | "h" | "?" => rt(self.lang, RtMsg::Help),
            "q" | "quit" | "exit" => return None,
            "b" | "break" | "breakpoint" => self.set_break(rest),
            "run" | "r" | "c" | "continue" => self.go(Mode::Continue),
            "n" | "next" => self.go(Mode::Over),
            "s" | "step" => self.go(Mode::Into),
            "o" | "out" | "finish" => self.go(Mode::Out),
            "v" | "vars" => self.vars(),
            "p" | "print" => self.print_var(rest),
            "bt" | "stack" => self.stack(),
            other => rt(self.lang, RtMsg::UnknownCmd { cmd: other.into() }),
        };
        let text = if text.ends_with('\n') || text.is_empty() {
            text
        } else {
            format!("{text}\n")
        };
        self.transcript.push_str(&text);
        Some(text)
    }

    fn set_break(&mut self, rest: &str) -> String {
        let Some((file, line)) = rest.rsplit_once(':') else {
            return rt(
                self.lang,
                RtMsg::UnknownCmd {
                    cmd: format!("b {rest}"),
                },
            );
        };
        let Ok(line) = line.trim().parse::<u32>() else {
            return rt(
                self.lang,
                RtMsg::UnknownCmd {
                    cmd: format!("b {rest}"),
                },
            );
        };
        let file = file.trim().replace('\\', "/");
        self.breaks.push((file.clone(), line));
        rt(self.lang, RtMsg::Breakpoint { file, line })
    }

    fn go(&mut self, mode: Mode) -> String {
        let start = self.site();
        let start_key = start.as_ref().map(site_key);
        let start_depth = self.vm.depth();
        let mut steps = 0u32;
        loop {
            if self.vm.finished() {
                return self.exited();
            }
            self.vm.step();
            steps += 1;
            if steps > STEP_LIMIT {
                return "step limit".into();
            }
            if self.vm.finished() {
                return self.exited();
            }
            let Some(site) = self.site() else {
                continue;
            };
            if !site.user {
                continue;
            }
            let key = site_key(&site);
            let stop = match mode {
                Mode::Continue => self.hits(&site) && Some(key) != start_key,
                Mode::Into => Some(key) != start_key,
                Mode::Over => self.vm.depth() <= start_depth && Some(key) != start_key,
                Mode::Out => self.vm.depth() < start_depth,
            };
            if stop {
                return self.stopped(&site, matches!(mode, Mode::Continue));
            }
        }
    }

    fn vars(&self) -> String {
        let mut text = rt(self.lang, RtMsg::VarsHeader);
        text.push('\n');
        let locals = self.vm.locals();
        if locals.is_empty() {
            text.push_str(&rt(self.lang, RtMsg::NoFrames));
            return text;
        }
        for (name, value) in locals {
            text.push_str(&format!("  {name} = {value}\n"));
        }
        text
    }

    fn print_var(&self, name: &str) -> String {
        let name = name.trim();
        match self.vm.lookup_local(name) {
            Some(v) => format!("{name} = {v}"),
            None => rt(self.lang, RtMsg::UnknownVar { name: name.into() }),
        }
    }

    fn stack(&self) -> String {
        let mut text = rt(self.lang, RtMsg::StackHeader);
        text.push('\n');
        let frames: Vec<_> = self
            .vm
            .backtrace()
            .into_iter()
            .filter(|f| f.func != "$entry")
            .collect();
        if frames.is_empty() {
            text.push_str(&rt(self.lang, RtMsg::NoFrames));
            return text;
        }
        for frame in frames {
            text.push_str(&format!("  {} ({}:{})\n", frame.func, frame.file, frame.line));
        }
        text
    }

    fn site(&self) -> Option<Site> {
        let view = self.vm.location()?;
        Some(Site {
            user: view.line > 0 && view.func != "$entry",
            func: view.func,
            file: view.file,
            line: view.line,
        })
    }

    fn hits(&self, site: &Site) -> bool {
        let file = site.file.replace('\\', "/");
        self.breaks.iter().any(|(pat, line)| {
            *line == site.line && (file == *pat || file.ends_with(pat) || file.ends_with(&format!("/{pat}")))
        })
    }

    fn stopped(&self, site: &Site, at_break: bool) -> String {
        let where_ = rt(
            self.lang,
            RtMsg::StepWhere {
                func: site.func.clone(),
                file: site.file.clone(),
                line: site.line,
            },
        );
        if at_break {
            let br = rt(
                self.lang,
                RtMsg::Breakpoint {
                    file: site.file.clone(),
                    line: site.line,
                },
            );
            format!("{br}\n{where_}")
        } else {
            where_
        }
    }

    fn exited(&self) -> String {
        rt(self.lang, RtMsg::Exited { code: self.vm.outcome().exit })
    }
}

fn site_key(site: &Site) -> (String, u32, String) {
    (site.file.clone(), site.line, site.func.clone())
}

fn split_cmd(input: &str) -> (&str, &str) {
    let input = input.trim();
    match input.split_once(char::is_whitespace) {
        Some((cmd, rest)) => (cmd, rest.trim()),
        None => (input, ""),
    }
}

pub fn repl(module: Module, args: &[String], lang: Lang) {
    use std::io::{BufRead, Write};
    println!("{}", rt(lang, RtMsg::Started));
    let mut dbg = Debugger::new(module, args, lang);
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        print!("{}", rt(lang, RtMsg::Prompt));
        let _ = std::io::stdout().flush();
        let Some(Ok(line)) = lines.next() else {
            break;
        };
        match dbg.command(&line) {
            None => break,
            Some(text) => {
                if !text.is_empty() {
                    print!("{text}");
                }
            }
        }
    }
}
