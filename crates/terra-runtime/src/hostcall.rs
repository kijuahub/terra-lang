//! Operating-system and data libraries behind `Op::Host`.
//!
//! Handles live on the same heap as objects. `delete` closes them. A thread
//! runs a Terra function in a fresh virtual machine; the payload is a string.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde_json::{Map, Number, Value as Json};
use sha1::{Digest as _, Sha1};
use sha2::Sha256;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use terra_bytecode::HostTy;

use crate::{Fault, Handle, SlotData, Value, Vm};

static RNG: Mutex<Option<StdRng>> = Mutex::new(None);

pub(crate) enum HostObj {
    List(Vec<Value>),
    Set(Vec<Value>),
    Stack(Vec<Value>),
    Queue(VecDeque<Value>),
    Map(Vec<(String, Value)>),
    Json(Json),
    Sqlite(Connection),
    Thread(Option<JoinHandle<Result<String, String>>>),
    Tcp(TcpStream),
    Listener(TcpListener),
    Ui(crate::ui::Node),
}

impl HostObj {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            HostObj::List(_) => "list",
            HostObj::Set(_) => "set",
            HostObj::Stack(_) => "stack",
            HostObj::Queue(_) => "queue",
            HostObj::Map(_) => "map",
            HostObj::Json(_) => "json",
            HostObj::Sqlite(_) => "sqlite",
            HostObj::Thread(_) => "thread",
            HostObj::Tcp(_) => "tcp",
            HostObj::Listener(_) => "listener",
            HostObj::Ui(_) => "ui",
        }
    }
}

pub(crate) fn exec(vm: &mut Vm, id: u16) -> Result<(), Fault> {
    let sig = terra_bytecode::host(id).ok_or_else(|| Fault::Type(format!("bad host {id}")))?;
    let args = vm.pop_n(sig.params.len())?;
    let value = dispatch(vm, sig.package, sig.name, args)?;
    if !matches!(sig.ret, HostTy::Void) {
        vm.push(value);
    }
    Ok(())
}

fn dispatch(vm: &mut Vm, package: &str, name: &str, args: Vec<Value>) -> Result<Value, Fault> {
    match (package, name) {
        ("com.terra.math", "sin") => num(f64(vm, &args, 0)?.sin()),
        ("com.terra.math", "cos") => num(f64(vm, &args, 0)?.cos()),
        ("com.terra.math", "tan") => num(f64(vm, &args, 0)?.tan()),
        ("com.terra.math", "asin") => num(f64(vm, &args, 0)?.asin()),
        ("com.terra.math", "acos") => num(f64(vm, &args, 0)?.acos()),
        ("com.terra.math", "atan") => num(f64(vm, &args, 0)?.atan()),
        ("com.terra.math", "atan2") => num(f64(vm, &args, 0)?.atan2(f64(vm, &args, 1)?)),
        ("com.terra.math", "sqrt") => num(f64(vm, &args, 0)?.sqrt()),
        ("com.terra.math", "pow") => num(f64(vm, &args, 0)?.powf(f64(vm, &args, 1)?)),
        ("com.terra.math", "log") => num(f64(vm, &args, 0)?.ln()),
        ("com.terra.math", "log10") => num(f64(vm, &args, 0)?.log10()),
        ("com.terra.math", "exp") => num(f64(vm, &args, 0)?.exp()),
        ("com.terra.math", "floor") => num(f64(vm, &args, 0)?.floor()),
        ("com.terra.math", "ceil") => num(f64(vm, &args, 0)?.ceil()),
        ("com.terra.math", "round") => num(f64(vm, &args, 0)?.round()),
        ("com.terra.math", "abs") => num(f64(vm, &args, 0)?.abs()),
        ("com.terra.math", "min") => num(f64(vm, &args, 0)?.min(f64(vm, &args, 1)?)),
        ("com.terra.math", "max") => num(f64(vm, &args, 0)?.max(f64(vm, &args, 1)?)),
        ("com.terra.math", "pi") => Ok(Value::Double(std::f64::consts::PI)),
        ("com.terra.math", "e") => Ok(Value::Double(std::f64::consts::E)),
        ("com.terra.fs", "read_text") => Ok(Value::Str(fs::read_to_string(text(&args, 0)?).map_err(io)?)),
        ("com.terra.fs", "write_text") => {
            fs::write(text(&args, 0)?, text(&args, 1)?).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.fs", "append_text") => {
            let mut file = fs::OpenOptions::new().create(true).append(true).open(text(&args, 0)?).map_err(io)?;
            file.write_all(text(&args, 1)?.as_bytes()).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.fs", "exists") => Ok(Value::Bool(Path::new(text(&args, 0)?).exists())),
        ("com.terra.fs", "is_file") => Ok(Value::Bool(Path::new(text(&args, 0)?).is_file())),
        ("com.terra.fs", "is_dir") => Ok(Value::Bool(Path::new(text(&args, 0)?).is_dir())),
        ("com.terra.fs", "remove") => {
            let path = Path::new(text(&args, 0)?);
            if path.is_dir() {
                fs::remove_dir_all(path).map_err(io)?;
            } else {
                fs::remove_file(path).map_err(io)?;
            }
            Ok(Value::Null)
        }
        ("com.terra.fs", "mkdir") => {
            fs::create_dir_all(text(&args, 0)?).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.fs", "list") => {
            let mut names = Vec::new();
            for entry in fs::read_dir(text(&args, 0)?).map_err(io)? {
                let entry = entry.map_err(io)?;
                names.push(Json::String(entry.file_name().to_string_lossy().into_owned()));
            }
            names.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            Ok(Value::Str(Json::Array(names).to_string()))
        }
        ("com.terra.fs", "copy") => {
            fs::copy(text(&args, 0)?, text(&args, 1)?).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.fs", "rename") => {
            fs::rename(text(&args, 0)?, text(&args, 1)?).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.fs", "size") => Ok(Value::Int(fs::metadata(text(&args, 0)?).map_err(io)?.len() as i64)),
        ("com.terra.fs", "read_base64") => Ok(Value::Str(b64_encode(&fs::read(text(&args, 0)?).map_err(io)?))),
        ("com.terra.fs", "write_base64") => {
            fs::write(text(&args, 0)?, b64_decode(text(&args, 1)?)?).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.env", "get") => Ok(Value::Str(std::env::var(text(&args, 0)?).unwrap_or_default())),
        ("com.terra.env", "has") => Ok(Value::Bool(std::env::var_os(text(&args, 0)?).is_some())),
        ("com.terra.env", "set") => {
            std::env::set_var(text(&args, 0)?, text(&args, 1)?);
            Ok(Value::Null)
        }
        ("com.terra.env", "remove") => {
            std::env::remove_var(text(&args, 0)?);
            Ok(Value::Null)
        }
        ("com.terra.env", "all") => {
            let mut obj = Map::new();
            for (k, v) in std::env::vars() {
                obj.insert(k, Json::String(v));
            }
            Ok(Value::Str(Json::Object(obj).to_string()))
        }
        ("com.terra.os", "name") => Ok(Value::Str(std::env::consts::OS.into())),
        ("com.terra.os", "arch") => Ok(Value::Str(std::env::consts::ARCH.into())),
        ("com.terra.os", "cwd") => Ok(Value::Str(std::env::current_dir().map_err(io)?.display().to_string())),
        ("com.terra.os", "chdir") => {
            std::env::set_current_dir(text(&args, 0)?).map_err(io)?;
            Ok(Value::Null)
        }
        ("com.terra.os", "home") => Ok(Value::Str(home_dir())),
        ("com.terra.os", "tmp") => Ok(Value::Str(std::env::temp_dir().display().to_string())),
        ("com.terra.os", "pid") => Ok(Value::Int(std::process::id() as i64)),
        ("com.terra.os", "hostname") => Ok(Value::Str(hostname())),
        ("com.terra.os", "sleep") => {
            std::thread::sleep(Duration::from_millis(int(&args, 0)?.max(0) as u64));
            Ok(Value::Null)
        }
        ("com.terra.os", "args") => Ok(alloc(vm, HostObj::List(program_args(vm)))),
        ("com.terra.path", "join") => Ok(Value::Str(Path::new(text(&args, 0)?).join(text(&args, 1)?).display().to_string())),
        ("com.terra.path", "name") => Ok(Value::Str(Path::new(text(&args, 0)?).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())),
        ("com.terra.path", "parent") => Ok(Value::Str(Path::new(text(&args, 0)?).parent().map(|p| p.display().to_string()).unwrap_or_default())),
        ("com.terra.path", "ext") => Ok(Value::Str(Path::new(text(&args, 0)?).extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())),
        ("com.terra.proc", "output") => proc_output(vm, text(&args, 0)?, host(&args, 1)?),
        ("com.terra.proc", "exit") => std::process::exit(int(&args, 0)? as i32),
        ("com.terra.http", "get") => http_get(text(&args, 0)?),
        ("com.terra.http", "text") => http_text(text(&args, 0)?, text(&args, 1)?, text(&args, 2)?, text(&args, 3)?),
        ("com.terra.http", "serve") => http_serve(vm, int(&args, 0)?, text(&args, 1)?, None),
        ("com.terra.http", "serve_n") => http_serve(vm, int(&args, 0)?, text(&args, 1)?, Some(int(&args, 2)?.max(0) as usize)),
        ("com.terra.net", "connect") => tcp_connect(vm, text(&args, 0)?, int(&args, 1)?),
        ("com.terra.net", "send") => tcp_send(vm, host(&args, 0)?, text(&args, 1)?),
        ("com.terra.net", "recv") => tcp_recv(vm, host(&args, 0)?, int(&args, 1)?),
        ("com.terra.net", "close") => {
            vm.heap.free(host(&args, 0)?)?;
            Ok(Value::Null)
        }
        ("com.terra.net", "listen") => tcp_listen(vm, int(&args, 0)?),
        ("com.terra.net", "port") => tcp_port(vm, host(&args, 0)?),
        ("com.terra.net", "accept") => tcp_accept(vm, host(&args, 0)?),
        ("com.terra.thread", "spawn") => thread_spawn(vm, text(&args, 0)?, text(&args, 1)?),
        ("com.terra.thread", "join") => thread_join(vm, host(&args, 0)?),
        ("com.terra.thread", "sleep") => {
            std::thread::sleep(Duration::from_millis(int(&args, 0)?.max(0) as u64));
            Ok(Value::Null)
        }
        ("com.terra.thread", "id") => Ok(Value::Int(thread_id())),
        ("com.terra.json", "parse") => Ok(alloc(vm, HostObj::Json(serde_json::from_str(text(&args, 0)?).map_err(io)?))),
        ("com.terra.json", "stringify") => Ok(Value::Str(value_to_json(vm, &args[0])?.to_string())),
        ("com.terra.json", "kind") => json_kind(vm, &args[0]),
        ("com.terra.json", "get") => json_get(vm, host(&args, 0)?, text(&args, 1)?),
        ("com.terra.json", "set") => json_set(vm, host(&args, 0)?, text(&args, 1)?, &args[2]),
        ("com.terra.json", "index") => json_index(vm, host(&args, 0)?, int(&args, 1)?),
        ("com.terra.json", "len") => json_len(vm, &args[0]),
        ("com.terra.json", "push") => json_push(vm, host(&args, 0)?, &args[1]),
        ("com.terra.xml", "parse") => Ok(alloc(vm, HostObj::Json(xml_to_json(text(&args, 0)?)?))),
        ("com.terra.xml", "stringify") => Ok(Value::Str(json_to_xml(&value_to_json(vm, &args[0])?))),
        ("com.terra.yaml", "parse") => {
            let yaml: serde_yaml::Value = serde_yaml::from_str(text(&args, 0)?).map_err(io)?;
            let json = serde_json::to_value(yaml).map_err(io)?;
            Ok(alloc(vm, HostObj::Json(json)))
        }
        ("com.terra.yaml", "stringify") => {
            let json = value_to_json(vm, &args[0])?;
            let yaml = serde_yaml::to_string(&json).map_err(io)?;
            Ok(Value::Str(yaml))
        }
        ("com.terra.sqlite", "open") => Ok(alloc(vm, HostObj::Sqlite(Connection::open(text(&args, 0)?).map_err(io)?))),
        ("com.terra.sqlite", "exec") => sqlite_exec(vm, host(&args, 0)?, text(&args, 1)?),
        ("com.terra.sqlite", "query") => sqlite_query(vm, host(&args, 0)?, text(&args, 1)?),
        ("com.terra.sqlite", "close") => {
            vm.heap.free(host(&args, 0)?)?;
            Ok(Value::Null)
        }
        ("com.terra.archive", "add") => {
            zip_add(text(&args, 0)?, text(&args, 1)?, text(&args, 2)?)?;
            Ok(Value::Null)
        }
        ("com.terra.archive", "list") => Ok(Value::Str(zip_list(text(&args, 0)?)?)),
        ("com.terra.archive", "read") => Ok(Value::Str(zip_read(text(&args, 0)?, text(&args, 1)?)?)),
        ("com.terra.archive", "extract") => {
            zip_extract(text(&args, 0)?, text(&args, 1)?)?;
            Ok(Value::Null)
        }
        ("com.terra.list", "new") => Ok(alloc(vm, HostObj::List(Vec::new()))),
        ("com.terra.list", "push") => {
            let h = host(&args, 0)?;
            list_push(vm, h, args.into_iter().nth(1).unwrap_or(Value::Null))
        }
        ("com.terra.list", "pop") => list_pop(vm, host(&args, 0)?),
        ("com.terra.list", "get") => list_get(vm, host(&args, 0)?, int(&args, 1)?),
        ("com.terra.list", "set") => {
            let h = host(&args, 0)?;
            let index = int(&args, 1)?;
            list_set(vm, h, index, args.into_iter().nth(2).unwrap_or(Value::Null))
        }
        ("com.terra.list", "len") => Ok(Value::Int(list_of(vm, host(&args, 0)?)?.len() as i64)),
        ("com.terra.list", "remove") => list_remove(vm, host(&args, 0)?, int(&args, 1)?),
        ("com.terra.list", "clear") => {
            list_of_mut(vm, host(&args, 0)?)?.clear();
            Ok(Value::Null)
        }
        ("com.terra.set", "new") => Ok(alloc(vm, HostObj::Set(Vec::new()))),
        ("com.terra.set", "add") => {
            let h = host(&args, 0)?;
            set_add(vm, h, args.into_iter().nth(1).unwrap_or(Value::Null))
        }
        ("com.terra.set", "has") => set_has(vm, host(&args, 0)?, &args[1]),
        ("com.terra.set", "remove") => set_remove(vm, host(&args, 0)?, &args[1]),
        ("com.terra.set", "len") => Ok(Value::Int(set_of(vm, host(&args, 0)?)?.len() as i64)),
        ("com.terra.stack", "new") => Ok(alloc(vm, HostObj::Stack(Vec::new()))),
        ("com.terra.stack", "push") => {
            let h = host(&args, 0)?;
            let value = args.into_iter().nth(1).unwrap_or(Value::Null);
            stack_of_mut(vm, h)?.push(value);
            Ok(Value::Null)
        }
        ("com.terra.stack", "pop") => stack_of_mut(vm, host(&args, 0)?)?.pop().ok_or_else(|| Fault::Io("empty stack".into())),
        ("com.terra.stack", "peek") => stack_of(vm, host(&args, 0)?)?.last().cloned().ok_or_else(|| Fault::Io("empty stack".into())),
        ("com.terra.stack", "len") => Ok(Value::Int(stack_of(vm, host(&args, 0)?)?.len() as i64)),
        ("com.terra.queue", "new") => Ok(alloc(vm, HostObj::Queue(VecDeque::new()))),
        ("com.terra.queue", "push") => {
            let h = host(&args, 0)?;
            let value = args.into_iter().nth(1).unwrap_or(Value::Null);
            queue_of_mut(vm, h)?.push_back(value);
            Ok(Value::Null)
        }
        ("com.terra.queue", "pop") => queue_of_mut(vm, host(&args, 0)?)?.pop_front().ok_or_else(|| Fault::Io("empty queue".into())),
        ("com.terra.queue", "len") => Ok(Value::Int(queue_of(vm, host(&args, 0)?)?.len() as i64)),
        ("com.terra.map", "new") => Ok(alloc(vm, HostObj::Map(Vec::new()))),
        ("com.terra.map", "set") => {
            let h = host(&args, 0)?;
            let key = text(&args, 1)?.to_string();
            map_set(vm, h, &key, args.into_iter().nth(2).unwrap_or(Value::Null))
        }
        ("com.terra.map", "get") => map_get(vm, host(&args, 0)?, text(&args, 1)?),
        ("com.terra.map", "has") => {
            let h = host(&args, 0)?;
            let key = text(&args, 1)?;
            Ok(Value::Bool(map_of(vm, h)?.iter().any(|(k, _)| k == key)))
        }
        ("com.terra.map", "remove") => {
            let key = text(&args, 1)?.to_string();
            map_of_mut(vm, host(&args, 0)?)?.retain(|(k, _)| k != &key);
            Ok(Value::Null)
        }
        ("com.terra.map", "len") => Ok(Value::Int(map_of(vm, host(&args, 0)?)?.len() as i64)),
        ("com.terra.map", "keys") => {
            let keys = map_of(vm, host(&args, 0)?)?.iter().map(|(k, _)| Value::Str(k.clone())).collect();
            Ok(alloc(vm, HostObj::List(keys)))
        }
        ("com.terra.hash", "sha256") => Ok(Value::Str(hex(&Sha256::digest(text(&args, 0)?.as_bytes())))),
        ("com.terra.hash", "sha1") => Ok(Value::Str(hex(&Sha1::digest(text(&args, 0)?.as_bytes())))),
        ("com.terra.hash", "md5") => Ok(Value::Str(hex(&md5::Md5::digest(text(&args, 0)?.as_bytes())))),
        ("com.terra.hash", "fnv") => Ok(Value::Str(format!("{:016x}", fnv(text(&args, 0)?)))),
        ("com.terra.time", "now") => Ok(Value::Int(now_ms())),
        ("com.terra.time", "format") => Ok(Value::Str(format_time(int(&args, 0)?, text(&args, 1)?))),
        ("com.terra.time", "parse") => Ok(Value::Int(parse_time(text(&args, 0)?)?)),
        ("com.terra.time", "parts") => Ok(Value::Str(time_parts(int(&args, 0)?).to_string())),
        ("com.terra.time", "add") => Ok(Value::Int(int(&args, 0)?.saturating_add(int(&args, 1)?))),
        ("com.terra.regex", "is_match") => Ok(Value::Bool(regex(text(&args, 0)?)?.is_match(text(&args, 1)?))),
        ("com.terra.regex", "find") => Ok(Value::Str(regex(text(&args, 0)?)?.find(text(&args, 1)?).map(|m| m.as_str().to_string()).unwrap_or_default())),
        ("com.terra.regex", "find_all") => {
            let found: Vec<Json> = regex(text(&args, 0)?)?.find_iter(text(&args, 1)?).map(|m| Json::String(m.as_str().into())).collect();
            Ok(Value::Str(Json::Array(found).to_string()))
        }
        ("com.terra.regex", "replace") => Ok(Value::Str(regex(text(&args, 0)?)?.replace_all(text(&args, 1)?, text(&args, 2)?).into_owned())),
        ("com.terra.regex", "split") => {
            let parts: Vec<Json> = regex(text(&args, 0)?)?.split(text(&args, 1)?).map(|s| Json::String(s.into())).collect();
            Ok(Value::Str(Json::Array(parts).to_string()))
        }
        ("com.terra.rand", "int") => rand_int(int(&args, 0)?, int(&args, 1)?),
        ("com.terra.rand", "float") => Ok(Value::Double(with_rng(|rng| rng.gen::<f64>()))),
        ("com.terra.rand", "seed") => {
            *RNG.lock().unwrap() = Some(StdRng::seed_from_u64(int(&args, 0)? as u64));
            Ok(Value::Null)
        }
        ("com.terra.codec", "encode") => Ok(Value::Str(b64_encode(text(&args, 0)?.as_bytes()))),
        ("com.terra.codec", "decode") => Ok(Value::Str(String::from_utf8(b64_decode(text(&args, 0)?)?).map_err(io)?)),
        ("com.terra.ui", name) => crate::ui::dispatch(vm, name, args),
        _ => Err(Fault::Type(format!("unimplemented host {package}.{name}"))),
    }
}

fn alloc(vm: &mut Vm, obj: HostObj) -> Value {
    Value::Host(vm.heap.alloc(SlotData::Host(obj)))
}

fn text<'a>(args: &'a [Value], i: usize) -> Result<&'a str, Fault> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        _ => Err(Fault::Type("string expected".into())),
    }
}

fn int(args: &[Value], i: usize) -> Result<i64, Fault> {
    match args.get(i) {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err(Fault::Type("int expected".into())),
    }
}

fn f64(vm: &Vm, args: &[Value], i: usize) -> Result<f64, Fault> {
    match args.get(i) {
        Some(Value::Double(n)) => Ok(*n),
        Some(Value::Float(n)) => Ok(*n as f64),
        Some(Value::Int(n)) => Ok(*n as f64),
        _ => Err(Fault::Type(format!("number expected at {i} in {}", vm.module.entry))),
    }
}

fn host(args: &[Value], i: usize) -> Result<Handle, Fault> {
    match args.get(i) {
        Some(Value::Host(h)) => Ok(*h),
        Some(Value::Null) => Err(Fault::NullRef),
        _ => Err(Fault::Type("handle expected".into())),
    }
}

fn num(n: f64) -> Result<Value, Fault> {
    if n.is_nan() {
        Err(Fault::Io("math domain error".into()))
    } else {
        Ok(Value::Double(n))
    }
}

fn fault_text(err: &Fault) -> String {
    match err {
        Fault::Io(s) | Fault::Type(s) | Fault::Cast(s) => s.clone(),
        Fault::DivZero => "division by zero".into(),
        Fault::NullRef => "null reference".into(),
        Fault::Index { len, index } => format!("index {index} of {len}"),
        _ => "request failed".into(),
    }
}

fn io(err: impl std::fmt::Display) -> Fault {
    Fault::Io(err.to_string())
}

fn program_args(vm: &Vm) -> Vec<Value> {
    let Some(Value::Arr(h)) = vm.globals.first() else {
        return Vec::new();
    };
    vm.heap
        .slot(*h)
        .ok()
        .and_then(|s| match &s.data {
            SlotData::Arr { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default()
}

fn hostname() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
        if !name.is_empty() {
            return name;
        }
    }
    Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".into())
}

fn proc_output(vm: &Vm, cmd: &str, args_h: Handle) -> Result<Value, Fault> {
    let mut command = Command::new(cmd);
    for arg in list_of(vm, args_h)? {
        let Value::Str(s) = arg else {
            return Err(Fault::Type("process arguments must be strings".into()));
        };
        command.arg(s);
    }
    let output = command.output().map_err(io)?;
    let body = Json::Object(Map::from_iter([
        ("code".into(), Json::from(output.status.code().unwrap_or(-1))),
        ("stdout".into(), Json::String(String::from_utf8_lossy(&output.stdout).into_owned())),
        ("stderr".into(), Json::String(String::from_utf8_lossy(&output.stderr).into_owned())),
    ]));
    Ok(Value::Str(body.to_string()))
}

fn http_get(url: &str) -> Result<Value, Fault> {
    let response = request("GET", url, "", "", 0)?;
    if response.status >= 400 {
        return Err(Fault::Io(format!("HTTP {} for {url}", response.status)));
    }
    Ok(Value::Str(response.body))
}

fn http_text(method: &str, url: &str, content_type: &str, body: &str) -> Result<Value, Fault> {
    let response = request(method, url, content_type, body, 0)?;
    let json = Json::Object(Map::from_iter([
        ("status".into(), Json::from(response.status)),
        ("content_type".into(), Json::String(response.content_type)),
        ("body".into(), Json::String(response.body)),
    ]));
    Ok(Value::Str(json.to_string()))
}

struct HttpResponse {
    status: i64,
    content_type: String,
    body: String,
}

fn request(method: &str, url: &str, content_type: &str, body: &str, redirects: u8) -> Result<HttpResponse, Fault> {
    if redirects > 5 {
        return Err(Fault::Io("too many redirects".into()));
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| Fault::Io("only http:// URLs are supported".into()))?;
    let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = format!("/{path}");
    let (host, port) = match hostport.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            (host, port.parse::<u16>().map_err(io)?)
        }
        _ => (hostport, 80),
    };
    let mut stream = TcpStream::connect_timeout(&format!("{host}:{port}").parse().map_err(io)?, Duration::from_secs(10)).map_err(io)?;
    stream.set_read_timeout(Some(Duration::from_secs(10))).map_err(io)?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(io)?;
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {hostport}\r\nConnection: close\r\nAccept: */*\r\n");
    if !body.is_empty() || method.eq_ignore_ascii_case("POST") || method.eq_ignore_ascii_case("PUT") {
        let ctype = if content_type.is_empty() { "text/plain" } else { content_type };
        head.push_str(&format!("Content-Type: {ctype}\r\nContent-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).map_err(io)?;
    stream.write_all(body.as_bytes()).map_err(io)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(io)?;
    let text = String::from_utf8_lossy(&raw);
    let (header, payload) = text.split_once("\r\n\r\n").unwrap_or((text.as_ref(), ""));
    let status_line = header.lines().next().unwrap_or("");
    let status = status_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut content_type = String::new();
    let mut location = String::new();
    for line in header.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-type") {
                content_type = v.trim().to_string();
            } else if k.eq_ignore_ascii_case("location") {
                location = v.trim().to_string();
            }
        }
    }
    if matches!(status, 301 | 302 | 303 | 307 | 308) && !location.is_empty() && method.eq_ignore_ascii_case("GET") {
        let next = if location.starts_with("http://") {
            location
        } else if let Some(path) = location.strip_prefix('/') {
            format!("http://{hostport}/{path}")
        } else {
            return Err(Fault::Io(format!("bad redirect {location}")));
        };
        return request("GET", &next, "", "", redirects + 1);
    }
    Ok(HttpResponse { status, content_type, body: payload.to_string() })
}

fn http_serve(vm: &Vm, port: i64, handler: &str, limit: Option<usize>) -> Result<Value, Fault> {
    if !(0..=65535).contains(&port) {
        return Err(Fault::Io(format!("bad port {port}")));
    }
    let listener = TcpListener::bind(("0.0.0.0", port as u16)).map_err(io)?;
    let mut served = 0usize;
    for conn in listener.incoming() {
        let mut stream = conn.map_err(io)?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let response = match read_http(&mut stream) {
            Ok((method, path, body)) => match call_handler(vm, handler, &method, &path, &body) {
                Ok(text) => text,
                Err(err) => format!("500 text/plain\n{err}"),
            },
            Err(err) => format!("400 text/plain\n{}", fault_text(&err)),
        };
        let (status, ctype, payload) = split_response(&response);
        let reason = reason(status);
        let bytes = payload.as_bytes();
        let head = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(bytes);
        served += 1;
        if limit == Some(served) {
            break;
        }
    }
    Ok(Value::Null)
}

fn call_handler(vm: &Vm, handler: &str, method: &str, path: &str, body: &str) -> Result<String, String> {
    let mut fresh = Vm::blank(vm.module.clone(), vm.lang);
    match fresh.call_named(handler, vec![Value::Str(method.into()), Value::Str(path.into()), Value::Str(body.into())]) {
        Ok(Value::Str(s)) => Ok(s),
        Ok(other) => Ok(fresh.show(&other, 0)),
        Err(Fault::Io(s)) => Err(s),
        Err(_) => Err("handler failed".into()),
    }
}

pub(crate) fn read_http(stream: &mut TcpStream) -> Result<(String, String, String), Fault> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let header_end;
    loop {
        let n = stream.read(&mut tmp).map_err(io)?;
        if n == 0 {
            return Err(Fault::Io("incomplete HTTP request".into()));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if buf.len() > 1024 * 1024 {
            return Err(Fault::Io("HTTP header is too large".into()));
        }
    }
    let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = header.lines();
    let request = lines.next().unwrap_or("");
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut length = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut tmp).map_err(io)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(length);
    Ok((method, path, String::from_utf8_lossy(&body).into_owned()))
}

fn split_response(text: &str) -> (u16, String, String) {
    if let Some((first, rest)) = text.split_once('\n') {
        let mut parts = first.trim_end_matches('\r').splitn(2, ' ');
        if let (Some(code), Some(ctype)) = (parts.next(), parts.next()) {
            if let Ok(code) = code.parse::<u16>() {
                if (100..600).contains(&code) && !ctype.trim().is_empty() {
                    return (code, ctype.trim().to_string(), rest.trim_start_matches('\r').to_string());
                }
            }
        }
    }
    (200, "text/plain; charset=utf-8".into(), text.to_string())
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn tcp_connect(vm: &mut Vm, host: &str, port: i64) -> Result<Value, Fault> {
    let stream = TcpStream::connect_timeout(&format!("{host}:{port}").parse().map_err(io)?, Duration::from_secs(10)).map_err(io)?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(io)?;
    stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(io)?;
    Ok(alloc(vm, HostObj::Tcp(stream)))
}

fn tcp_send(vm: &mut Vm, h: Handle, text: &str) -> Result<Value, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Tcp(stream)) => {
            stream.write_all(text.as_bytes()).map_err(io)?;
            Ok(Value::Null)
        }
        _ => Err(Fault::Type("not a socket".into())),
    }
}

fn tcp_recv(vm: &mut Vm, h: Handle, max: i64) -> Result<Value, Fault> {
    let max = max.clamp(1, 1_048_576) as usize;
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Tcp(stream)) => {
            let mut buf = vec![0u8; max];
            let n = match stream.read(&mut buf) {
                Ok(n) => n,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::TimedOut => 0,
                Err(err) => return Err(io(err)),
            };
            buf.truncate(n);
            Ok(Value::Str(String::from_utf8_lossy(&buf).into_owned()))
        }
        _ => Err(Fault::Type("not a socket".into())),
    }
}

fn tcp_listen(vm: &mut Vm, port: i64) -> Result<Value, Fault> {
    if !(0..=65535).contains(&port) {
        return Err(Fault::Io(format!("bad port {port}")));
    }
    let listener = TcpListener::bind(("0.0.0.0", port as u16)).map_err(io)?;
    Ok(alloc(vm, HostObj::Listener(listener)))
}

fn tcp_port(vm: &Vm, h: Handle) -> Result<Value, Fault> {
    let slot = vm.heap.slot(h)?;
    let port = match &slot.data {
        SlotData::Host(HostObj::Listener(listener)) => listener.local_addr().map_err(io)?.port(),
        SlotData::Host(HostObj::Tcp(stream)) => stream.local_addr().map_err(io)?.port(),
        _ => return Err(Fault::Type("not a socket".into())),
    };
    Ok(Value::Int(port as i64))
}

fn tcp_accept(vm: &mut Vm, h: Handle) -> Result<Value, Fault> {
    let stream = {
        let slot = vm.heap.slot_mut(h)?;
        match &mut slot.data {
            SlotData::Host(HostObj::Listener(listener)) => listener.accept().map_err(io)?.0,
            _ => return Err(Fault::Type("not a listener".into())),
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(io)?;
    stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(io)?;
    Ok(alloc(vm, HostObj::Tcp(stream)))
}

fn thread_spawn(vm: &mut Vm, name: &str, arg: &str) -> Result<Value, Fault> {
    let module = vm.module.clone();
    let lang = vm.lang;
    let name = name.to_string();
    let arg = arg.to_string();
    let handle = std::thread::spawn(move || {
        let mut worker = Vm::blank(module, lang);
        match worker.call_named(&name, vec![Value::Str(arg)]) {
            Ok(Value::Str(s)) => Ok(s),
            Ok(other) => Ok(worker.show(&other, 0)),
            Err(Fault::Io(s)) => Err(s),
            Err(_) => Err("thread failed".into()),
        }
    });
    Ok(alloc(vm, HostObj::Thread(Some(handle))))
}

fn thread_join(vm: &mut Vm, h: Handle) -> Result<Value, Fault> {
    let handle = {
        let slot = vm.heap.slot_mut(h)?;
        match &mut slot.data {
            SlotData::Host(HostObj::Thread(slot)) => slot.take(),
            _ => return Err(Fault::Type("not a thread".into())),
        }
    };
    let handle = handle.ok_or_else(|| Fault::Io("thread already joined".into()))?;
    let value = match handle.join() {
        Ok(Ok(text)) => Value::Str(text),
        Ok(Err(err)) => return Err(Fault::Io(err)),
        Err(_) => return Err(Fault::Io("thread panicked".into())),
    };
    let _ = vm.heap.free(h);
    Ok(value)
}

fn thread_id() -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    hasher.finish() as i64
}

fn value_to_json(vm: &Vm, value: &Value) -> Result<Json, Fault> {
    match value {
        Value::Null => Ok(Json::Null),
        Value::Bool(b) => Ok(Json::Bool(*b)),
        Value::Int(n) => Ok(Json::Number((*n).into())),
        Value::Float(n) => Number::from_f64(*n as f64).map(Json::Number).ok_or_else(|| Fault::Io("bad float".into())),
        Value::Double(n) => Number::from_f64(*n).map(Json::Number).ok_or_else(|| Fault::Io("bad number".into())),
        Value::Str(s) => Ok(Json::String(s.clone())),
        Value::Host(h) => {
            let slot = vm.heap.slot(*h)?;
            let owned = match &slot.data {
                SlotData::Host(HostObj::Json(v)) => return Ok(v.clone()),
                SlotData::Host(HostObj::List(xs)) => HostObj::List(xs.clone()),
                SlotData::Host(HostObj::Map(xs)) => HostObj::Map(xs.clone()),
                _ => return Err(Fault::Type("value cannot be written as JSON".into())),
            };
            let _ = slot;
            match owned {
                HostObj::List(xs) => {
                    let mut out = Vec::new();
                    for item in xs {
                        out.push(value_to_json(vm, &item)?);
                    }
                    Ok(Json::Array(out))
                }
                HostObj::Map(xs) => {
                    let mut obj = Map::new();
                    for (k, v) in xs {
                        obj.insert(k, value_to_json(vm, &v)?);
                    }
                    Ok(Json::Object(obj))
                }
                _ => unreachable!(),
            }
        }
        _ => Err(Fault::Type("value cannot be written as JSON".into())),
    }
}

fn json_kind(vm: &Vm, value: &Value) -> Result<Value, Fault> {
    let kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) | Value::Float(_) | Value::Double(_) => "number",
        Value::Str(_) => "string",
        Value::Host(h) => {
            let slot = vm.heap.slot(*h)?;
            match &slot.data {
                SlotData::Host(HostObj::Json(v)) => match v {
                    Json::Null => "null",
                    Json::Bool(_) => "bool",
                    Json::Number(_) => "number",
                    Json::String(_) => "string",
                    Json::Array(_) => "array",
                    Json::Object(_) => "object",
                },
                SlotData::Host(HostObj::List(_)) => "array",
                SlotData::Host(HostObj::Map(_)) => "object",
                _ => return Err(Fault::Type("not JSON".into())),
            }
        }
        _ => return Err(Fault::Type("not JSON".into())),
    };
    Ok(Value::Str(kind.into()))
}

fn json_value(vm: &Vm, h: Handle) -> Result<Json, Fault> {
    let slot = vm.heap.slot(h)?;
    match &slot.data {
        SlotData::Host(HostObj::Json(v)) => Ok(v.clone()),
        _ => Err(Fault::Type("not JSON".into())),
    }
}

fn json_get(vm: &mut Vm, h: Handle, key: &str) -> Result<Value, Fault> {
    let value = json_value(vm, h)?;
    let found = value.get(key).cloned().unwrap_or(Json::Null);
    Ok(json_to_terra(vm, found))
}

fn json_set(vm: &mut Vm, h: Handle, key: &str, value: &Value) -> Result<Value, Fault> {
    let encoded = value_to_json(vm, value)?;
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Json(Json::Object(obj))) => {
            obj.insert(key.to_string(), encoded);
            Ok(Value::Null)
        }
        _ => Err(Fault::Type("JSON object expected".into())),
    }
}

fn json_index(vm: &mut Vm, h: Handle, index: i64) -> Result<Value, Fault> {
    let value = json_value(vm, h)?;
    let Some(items) = value.as_array() else {
        return Err(Fault::Type("JSON array expected".into()));
    };
    if index < 0 || index as usize >= items.len() {
        return Err(Fault::Index { len: items.len() as i64, index });
    }
    Ok(json_to_terra(vm, items[index as usize].clone()))
}

fn json_len(vm: &Vm, value: &Value) -> Result<Value, Fault> {
    let n = match value {
        Value::Host(h) => {
            let slot = vm.heap.slot(*h)?;
            match &slot.data {
                SlotData::Host(HostObj::Json(Json::Array(xs))) => xs.len(),
                SlotData::Host(HostObj::Json(Json::Object(xs))) => xs.len(),
                SlotData::Host(HostObj::List(xs)) => xs.len(),
                SlotData::Host(HostObj::Map(xs)) => xs.len(),
                _ => return Err(Fault::Type("length expects JSON, a list, or a map".into())),
            }
        }
        _ => return Err(Fault::Type("length expects JSON, a list, or a map".into())),
    };
    Ok(Value::Int(n as i64))
}

fn json_push(vm: &mut Vm, h: Handle, value: &Value) -> Result<Value, Fault> {
    let encoded = value_to_json(vm, value)?;
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Json(Json::Array(xs))) => {
            xs.push(encoded);
            Ok(Value::Null)
        }
        _ => Err(Fault::Type("JSON array expected".into())),
    }
}

fn json_to_terra(vm: &mut Vm, value: Json) -> Value {
    match value {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(b),
        Json::Number(n) => n.as_i64().map(Value::Int).or_else(|| n.as_f64().map(Value::Double)).unwrap_or(Value::Null),
        Json::String(s) => Value::Str(s),
        other => alloc(vm, HostObj::Json(other)),
    }
}

fn xml_to_json(text: &str) -> Result<Json, Fault> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(XmlNode::from_event(&e)?),
            Ok(Event::Empty(e)) => {
                let node = XmlNode::from_event(&e)?;
                push_xml(&mut stack, &mut root, node)?;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(io)?.into_owned();
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                }
            }
            Ok(Event::End(_)) => {
                let node = stack.pop().ok_or_else(|| Fault::Io("unbalanced XML".into()))?;
                push_xml(&mut stack, &mut root, node)?;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(io(err)),
            _ => {}
        }
    }
    root.ok_or_else(|| Fault::Io("XML has no element".into()))
}

struct XmlNode {
    tag: String,
    attrs: Map<String, Json>,
    text: String,
    children: Vec<Json>,
}

impl XmlNode {
    fn from_event(e: &quick_xml::events::BytesStart<'_>) -> Result<Self, Fault> {
        let tag = String::from_utf8(e.name().as_ref().to_vec()).map_err(io)?;
        let mut attrs = Map::new();
        for attr in e.attributes() {
            let attr = attr.map_err(io)?;
            let key = String::from_utf8(attr.key.as_ref().to_vec()).map_err(io)?;
            let value = attr.unescape_value().map_err(io)?.into_owned();
            attrs.insert(key, Json::String(value));
        }
        Ok(Self { tag, attrs, text: String::new(), children: Vec::new() })
    }

    fn to_json(self) -> Json {
        Json::Object(Map::from_iter([
            ("tag".into(), Json::String(self.tag)),
            ("attrs".into(), Json::Object(self.attrs)),
            ("text".into(), Json::String(self.text)),
            ("children".into(), Json::Array(self.children)),
        ]))
    }
}

fn push_xml(stack: &mut Vec<XmlNode>, root: &mut Option<Json>, node: XmlNode) -> Result<(), Fault> {
    let json = node.to_json();
    if let Some(parent) = stack.last_mut() {
        parent.children.push(json);
    } else if root.is_none() {
        *root = Some(json);
    } else {
        return Err(Fault::Io("XML has more than one root".into()));
    }
    Ok(())
}

fn json_to_xml(value: &Json) -> String {
    let mut out = String::new();
    write_xml(&mut out, value);
    out
}

fn write_xml(out: &mut String, value: &Json) {
    let Some(obj) = value.as_object() else {
        out.push_str(&xml_escape(&value.to_string()));
        return;
    };
    let tag = obj.get("tag").and_then(|v| v.as_str()).unwrap_or("value");
    out.push('<');
    out.push_str(tag);
    if let Some(attrs) = obj.get("attrs").and_then(|v| v.as_object()) {
        for (k, v) in attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            out.push_str(&xml_escape(v.as_str().unwrap_or("")));
            out.push('"');
        }
    }
    let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let children = obj.get("children").and_then(|v| v.as_array());
    if text.is_empty() && children.map(|c| c.is_empty()).unwrap_or(true) {
        out.push_str("/>");
        return;
    }
    out.push('>');
    out.push_str(&xml_escape(text));
    if let Some(children) = children {
        for child in children {
            write_xml(out, child);
        }
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn sqlite_exec(vm: &mut Vm, h: Handle, sql: &str) -> Result<Value, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Sqlite(conn)) => {
            conn.execute_batch(sql).map_err(io)?;
            Ok(Value::Null)
        }
        _ => Err(Fault::Type("not a database".into())),
    }
}

fn sqlite_query(vm: &Vm, h: Handle, sql: &str) -> Result<Value, Fault> {
    let slot = vm.heap.slot(h)?;
    let SlotData::Host(HostObj::Sqlite(conn)) = &slot.data else {
        return Err(Fault::Type("not a database".into()));
    };
    let mut stmt = conn.prepare(sql).map_err(io)?;
    let cols: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_string()).collect();
    let mut rows = stmt.query([]).map_err(io)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(io)? {
        let mut obj = Map::new();
        for (i, name) in cols.iter().enumerate() {
            let value = match row.get_ref(i).map_err(io)? {
                ValueRef::Null => Json::Null,
                ValueRef::Integer(n) => Json::from(n),
                ValueRef::Real(n) => Number::from_f64(n).map(Json::Number).unwrap_or(Json::Null),
                ValueRef::Text(t) => Json::String(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(b) => Json::String(b64_encode(b)),
            };
            obj.insert(name.clone(), value);
        }
        out.push(Json::Object(obj));
    }
    Ok(Value::Str(Json::Array(out).to_string()))
}

fn zip_add(path: &str, name: &str, content: &str) -> Result<(), Fault> {
    reject_zip_name(name)?;
    let mut files = if Path::new(path).is_file() {
        read_zip(path)?
    } else {
        Vec::new()
    };
    files.retain(|(n, _)| n != name);
    files.push((name.to_string(), content.as_bytes().to_vec()));
    write_zip(path, &files)
}

fn zip_list(path: &str) -> Result<String, Fault> {
    let names: Vec<Json> = read_zip(path)?.into_iter().map(|(n, _)| Json::String(n)).collect();
    Ok(Json::Array(names).to_string())
}

fn zip_read(path: &str, name: &str) -> Result<String, Fault> {
    read_zip(path)?
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, bytes)| String::from_utf8_lossy(&bytes).into_owned())
        .ok_or_else(|| Fault::Io(format!("zip entry '{name}' was not found")))
}

fn zip_extract(path: &str, dest: &str) -> Result<(), Fault> {
    fs::create_dir_all(dest).map_err(io)?;
    for (name, bytes) in read_zip(path)? {
        let out = Path::new(dest).join(&name);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        fs::write(out, bytes).map_err(io)?;
    }
    Ok(())
}

fn read_zip(path: &str) -> Result<Vec<(String, Vec<u8>)>, Fault> {
    let file = File::open(path).map_err(io)?;
    let mut archive = ZipArchive::new(file).map_err(io)?;
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(io)?;
        let name = entry.name().to_string();
        reject_zip_name(&name)?;
        if name.ends_with('/') {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(io)?;
        out.push((name, bytes));
    }
    Ok(out)
}

fn write_zip(path: &str, files: &[(String, Vec<u8>)]) -> Result<(), Fault> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(io)?;
        }
    }
    let file = File::create(path).map_err(io)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in files {
        zip.start_file(name, opts).map_err(io)?;
        zip.write_all(bytes).map_err(io)?;
    }
    zip.finish().map_err(io)?;
    Ok(())
}

fn reject_zip_name(name: &str) -> Result<(), Fault> {
    if name.is_empty() || name.contains('\\') || name.split('/').any(|p| p == "..") {
        return Err(Fault::Io(format!("unsafe zip name '{name}'")));
    }
    Ok(())
}

fn list_of(vm: &Vm, h: Handle) -> Result<Vec<Value>, Fault> {
    let slot = vm.heap.slot(h)?;
    match &slot.data {
        SlotData::Host(HostObj::List(xs)) => Ok(xs.clone()),
        _ => Err(Fault::Type("not a list".into())),
    }
}

fn list_of_mut(vm: &mut Vm, h: Handle) -> Result<&mut Vec<Value>, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::List(xs)) => Ok(xs),
        _ => Err(Fault::Type("not a list".into())),
    }
}

fn list_push(vm: &mut Vm, h: Handle, value: Value) -> Result<Value, Fault> {
    list_of_mut(vm, h)?.push(value);
    Ok(Value::Null)
}

fn list_pop(vm: &mut Vm, h: Handle) -> Result<Value, Fault> {
    list_of_mut(vm, h)?.pop().ok_or_else(|| Fault::Io("empty list".into()))
}

fn list_get(vm: &Vm, h: Handle, index: i64) -> Result<Value, Fault> {
    let xs = list_of(vm, h)?;
    at(&xs, index).cloned()
}

fn list_set(vm: &mut Vm, h: Handle, index: i64, value: Value) -> Result<Value, Fault> {
    let xs = list_of_mut(vm, h)?;
    let len = xs.len();
    let slot = at_mut(xs, index, len)?;
    *slot = value;
    Ok(Value::Null)
}

fn list_remove(vm: &mut Vm, h: Handle, index: i64) -> Result<Value, Fault> {
    let xs = list_of_mut(vm, h)?;
    if index < 0 || index as usize >= xs.len() {
        return Err(Fault::Index { len: xs.len() as i64, index });
    }
    Ok(xs.remove(index as usize))
}

fn set_of(vm: &Vm, h: Handle) -> Result<Vec<Value>, Fault> {
    let slot = vm.heap.slot(h)?;
    match &slot.data {
        SlotData::Host(HostObj::Set(xs)) => Ok(xs.clone()),
        _ => Err(Fault::Type("not a set".into())),
    }
}

fn set_add(vm: &mut Vm, h: Handle, value: Value) -> Result<Value, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Set(xs)) => {
            if !xs.iter().any(|v| crate::values_eq(v, &value)) {
                xs.push(value);
            }
            Ok(Value::Null)
        }
        _ => Err(Fault::Type("not a set".into())),
    }
}

fn set_has(vm: &Vm, h: Handle, value: &Value) -> Result<Value, Fault> {
    Ok(Value::Bool(set_of(vm, h)?.iter().any(|v| crate::values_eq(v, value))))
}

fn set_remove(vm: &mut Vm, h: Handle, value: &Value) -> Result<Value, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Set(xs)) => {
            xs.retain(|v| !crate::values_eq(v, value));
            Ok(Value::Null)
        }
        _ => Err(Fault::Type("not a set".into())),
    }
}

fn stack_of(vm: &Vm, h: Handle) -> Result<Vec<Value>, Fault> {
    let slot = vm.heap.slot(h)?;
    match &slot.data {
        SlotData::Host(HostObj::Stack(xs)) => Ok(xs.clone()),
        _ => Err(Fault::Type("not a stack".into())),
    }
}

fn stack_of_mut(vm: &mut Vm, h: Handle) -> Result<&mut Vec<Value>, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Stack(xs)) => Ok(xs),
        _ => Err(Fault::Type("not a stack".into())),
    }
}

fn queue_of(vm: &Vm, h: Handle) -> Result<VecDeque<Value>, Fault> {
    let slot = vm.heap.slot(h)?;
    match &slot.data {
        SlotData::Host(HostObj::Queue(xs)) => Ok(xs.clone()),
        _ => Err(Fault::Type("not a queue".into())),
    }
}

fn queue_of_mut(vm: &mut Vm, h: Handle) -> Result<&mut VecDeque<Value>, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Queue(xs)) => Ok(xs),
        _ => Err(Fault::Type("not a queue".into())),
    }
}

fn map_of(vm: &Vm, h: Handle) -> Result<Vec<(String, Value)>, Fault> {
    let slot = vm.heap.slot(h)?;
    match &slot.data {
        SlotData::Host(HostObj::Map(xs)) => Ok(xs.clone()),
        _ => Err(Fault::Type("not a map".into())),
    }
}

fn map_of_mut(vm: &mut Vm, h: Handle) -> Result<&mut Vec<(String, Value)>, Fault> {
    let slot = vm.heap.slot_mut(h)?;
    match &mut slot.data {
        SlotData::Host(HostObj::Map(xs)) => Ok(xs),
        _ => Err(Fault::Type("not a map".into())),
    }
}

fn map_set(vm: &mut Vm, h: Handle, key: &str, value: Value) -> Result<Value, Fault> {
    let map = map_of_mut(vm, h)?;
    if let Some((_, slot)) = map.iter_mut().find(|(k, _)| k == key) {
        *slot = value;
    } else {
        map.push((key.to_string(), value));
    }
    Ok(Value::Null)
}

fn map_get(vm: &Vm, h: Handle, key: &str) -> Result<Value, Fault> {
    Ok(map_of(vm, h)?.into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or(Value::Null))
}

fn at(xs: &[Value], index: i64) -> Result<&Value, Fault> {
    if index < 0 || index as usize >= xs.len() {
        Err(Fault::Index { len: xs.len() as i64, index })
    } else {
        Ok(&xs[index as usize])
    }
}

fn at_mut(xs: &mut [Value], index: i64, len: usize) -> Result<&mut Value, Fault> {
    if index < 0 || index as usize >= len {
        Err(Fault::Index { len: len as i64, index })
    } else {
        Ok(&mut xs[index as usize])
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fnv(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in text.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn format_time(ms: i64, pattern: &str) -> String {
    let (y, m, d, hh, mm, ss) = parts(ms);
    pattern
        .replace("YYYY", &format!("{y:04}"))
        .replace("MM", &format!("{m:02}"))
        .replace("DD", &format!("{d:02}"))
        .replace("hh", &format!("{hh:02}"))
        .replace("mm", &format!("{mm:02}"))
        .replace("ss", &format!("{ss:02}"))
}

fn time_parts(ms: i64) -> Json {
    let (y, m, d, hh, mm, ss) = parts(ms);
    Json::Object(Map::from_iter([
        ("year".into(), Json::from(y)),
        ("month".into(), Json::from(m)),
        ("day".into(), Json::from(d)),
        ("hour".into(), Json::from(hh)),
        ("minute".into(), Json::from(mm)),
        ("second".into(), Json::from(ss)),
    ]))
}

fn parts(ms: i64) -> (i32, u32, u32, u32, u32, u32) {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400) as u32;
    let (y, m, d) = civil_from_days(days);
    (y, m, d, tod / 3600, (tod / 60) % 60, tod % 60)
}

fn parse_time(text: &str) -> Result<i64, Fault> {
    let text = text.trim().trim_end_matches('Z');
    let (date, time) = text.split_once('T').or_else(|| text.split_once(' ')).unwrap_or((text, "00:00:00"));
    let mut date = date.split('-');
    let year: i32 = date.next().unwrap_or("").parse().map_err(io)?;
    let month: u32 = date.next().unwrap_or("").parse().map_err(io)?;
    let day: u32 = date.next().unwrap_or("").parse().map_err(io)?;
    let mut time = time.split(':');
    let hour: u32 = time.next().unwrap_or("0").parse().map_err(io)?;
    let minute: u32 = time.next().unwrap_or("0").parse().map_err(io)?;
    let second: u32 = time.next().unwrap_or("0").chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return Err(Fault::Io(format!("bad time '{text}'")));
    }
    let days = days_from_civil(year, month, day);
    Ok(days * 86_400_000 + hour as i64 * 3_600_000 + minute as i64 * 60_000 + second as i64 * 1000)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let month = if month > 2 { month - 3 } else { month + 9 } as u64;
    let doy = (153 * month + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn regex(pattern: &str) -> Result<regex::Regex, Fault> {
    regex::Regex::new(pattern).map_err(io)
}

fn rand_int(lo: i64, hi: i64) -> Result<Value, Fault> {
    if lo > hi {
        return Err(Fault::Io(format!("rand_int range {lo}..{hi} is empty")));
    }
    Ok(Value::Int(with_rng(|rng| rng.gen_range(lo..=hi))))
}

fn with_rng<T>(f: impl FnOnce(&mut StdRng) -> T) -> T {
    let mut guard = RNG.lock().unwrap();
    if guard.is_none() {
        *guard = Some(StdRng::from_entropy());
    }
    f(guard.as_mut().unwrap())
}

fn b64_encode(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | bytes[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if bytes.len() - i == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if bytes.len() - i == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn b64_decode(text: &str) -> Result<Vec<u8>, Fault> {
    fn val(c: u8) -> Result<u8, Fault> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(Fault::Io("bad base64".into())),
        })
    }
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return Err(Fault::Io("bad base64 length".into()));
    }
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().filter(|c| **c == b'=').count();
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = if chunk[2] == b'=' { 0 } else { val(chunk[2])? };
        let d = if chunk[3] == b'=' { 0 } else { val(chunk[3])? };
        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | d as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}
