//! Stack virtual machine for Terra bytecode.
//!
//! Objects and arrays live on a manual heap addressed by generation handles.
//! `delete` frees one object; freeing it again is a double free, and using the
//! old handle is use-after-free. Tasks are runtime-owned and cannot be deleted.
//! Async is cooperative: `await` runs the task on this thread until it finishes.
//!
//! shortcut: integer overflow throws; float overflow becomes infinity, upgrade
//! when checked float semantics are required.

mod hostcall;

use std::collections::HashMap;

use terra_bytecode::{Constant, Module, Op};
use terra_diagnostics::{rt, Lang, RtMsg};

#[derive(Clone, Debug)]
pub struct Outcome {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    Exited(i32),
}

#[derive(Clone, Debug)]
pub struct FrameView {
    pub func: String,
    pub file: String,
    pub line: u32,
    pub col: u32,
}

pub fn execute(module: &Module, args: &[String], lang: Lang, echo: bool) -> Outcome {
    let mut vm = Vm::new(module.clone(), args, lang, echo);
    vm.run();
    vm.outcome()
}

pub fn lang_of(module: &Module) -> Lang {
    Lang::parse(&module.lang).unwrap_or(Lang::En)
}

#[derive(Clone, Debug)]
enum Value {
    Int(i64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Null,
    Obj(Handle),
    Arr(Handle),
    Task(Handle),
    Enum { id: u32, variant: u16 },
    Range { lo: i64, hi: i64 },
    Host(Handle),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Handle {
    index: u32,
    gen: u32,
}

enum Fault {
    DivZero,
    NullRef,
    UseAfterFree,
    DoubleFree,
    Index { len: i64, index: i64 },
    Overflow,
    Type(String),
    Cast(String),
    Io(String),
}

#[derive(Clone, Debug)]
enum TaskKind {
    Direct(u32),
    Virtual { slot: u16 },
    Interface { iface: u32, slot: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskState {
    Fresh,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug)]
struct Task {
    kind: TaskKind,
    args: Vec<Value>,
    state: TaskState,
    result: Option<Value>,
    error: Option<Value>,
    returns_void: bool,
    slot: u16,
}

struct Slot {
    gen: u32,
    data: SlotData,
}

enum SlotData {
    Empty,
    Obj {
        class: u32,
        fields: Vec<Value>,
        user: bool,
    },
    Arr {
        elems: Vec<Value>,
        user: bool,
    },
    Task(Task),
    Host(hostcall::HostObj),
}

struct Heap {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl Heap {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    fn alloc(&mut self, data: SlotData) -> Handle {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            let gen = slot.gen;
            slot.data = data;
            Handle { index, gen }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot { gen: 1, data });
            Handle { index, gen: 1 }
        }
    }

    fn slot(&self, h: Handle) -> Result<&Slot, Fault> {
        let slot = self.slots.get(h.index as usize).ok_or(Fault::UseAfterFree)?;
        if slot.gen != h.gen || matches!(slot.data, SlotData::Empty) {
            return Err(Fault::UseAfterFree);
        }
        Ok(slot)
    }

    fn slot_mut(&mut self, h: Handle) -> Result<&mut Slot, Fault> {
        let slot = self.slots.get_mut(h.index as usize).ok_or(Fault::UseAfterFree)?;
        if slot.gen != h.gen || matches!(slot.data, SlotData::Empty) {
            return Err(Fault::UseAfterFree);
        }
        Ok(slot)
    }

    fn free(&mut self, h: Handle) -> Result<(), Fault> {
        let Some(slot) = self.slots.get_mut(h.index as usize) else {
            return Err(Fault::DoubleFree);
        };
        if slot.gen != h.gen || matches!(slot.data, SlotData::Empty) {
            return Err(Fault::DoubleFree);
        }
        if matches!(slot.data, SlotData::Task(_)) {
            return Err(Fault::Type("cannot delete a task".into()));
        }
        slot.gen = slot.gen.wrapping_add(1);
        slot.data = SlotData::Empty;
        self.free.push(h.index);
        Ok(())
    }

    fn user_live(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| {
                matches!(
                    s.data,
                    SlotData::Obj { user: true, .. } | SlotData::Arr { user: true, .. }
                )
            })
            .count()
    }
}

struct Handler {
    ip: u32,
}

struct Frame {
    func: u32,
    ip: u32,
    locals: Vec<Value>,
    stack: Vec<Value>,
    handlers: Vec<Handler>,
    task: Option<Handle>,
}

pub struct Vm {
    module: Module,
    frames: Vec<Frame>,
    heap: Heap,
    globals: Vec<Value>,
    statics: HashMap<(u32, u16), Value>,
    stdout: String,
    stderr: String,
    lang: Lang,
    echo: bool,
    exit: Option<i32>,
    leaks_reported: bool,
    last_value: Option<Value>,
}

impl Vm {
    pub fn new(module: Module, args: &[String], lang: Lang, echo: bool) -> Self {
        let mut vm = Self {
            module,
            frames: Vec::new(),
            heap: Heap::new(),
            globals: Vec::new(),
            statics: HashMap::new(),
            stdout: String::new(),
            stderr: String::new(),
            lang,
            echo,
            exit: None,
            leaks_reported: false,
            last_value: None,
        };
        vm.boot(args);
        vm
    }

    pub(crate) fn blank(module: Module, lang: Lang) -> Self {
        let mut vm = Self {
            module,
            frames: Vec::new(),
            heap: Heap::new(),
            globals: Vec::new(),
            statics: HashMap::new(),
            stdout: String::new(),
            stderr: String::new(),
            lang,
            echo: false,
            exit: None,
            leaks_reported: false,
            last_value: None,
        };
        let args_h = vm.heap.alloc(SlotData::Arr {
            elems: Vec::new(),
            user: false,
        });
        vm.globals = vec![Value::Arr(args_h)];
        vm
    }

    pub(crate) fn call_named(&mut self, name: &str, args: Vec<Value>) -> Result<Value, Fault> {
        let func = self
            .module
            .functions
            .iter()
            .position(|f| f.name == name)
            .ok_or_else(|| Fault::Io(format!("no function '{name}'")))? as u32;
        let arity = self.module.functions[func as usize].arity;
        if arity as usize != args.len() {
            return Err(Fault::Io(format!("'{name}' expects {arity} arguments")));
        }
        let depth = self.frames.len();
        self.last_value = None;
        self.push_frame(func, args, None);
        while self.frames.len() > depth && self.exit.is_none() {
            self.step();
        }
        if depth == 0 {
            if self.last_value.is_none() && self.exit == Some(1) {
                return Err(Fault::Io(self.stderr.trim().to_string()));
            }
            return Ok(self.last_value.take().unwrap_or(Value::Null));
        }
        self.last_value = None;
        self.pop()
    }

    fn boot(&mut self, args: &[String]) {
        let inits: Vec<(u32, u16, Constant)> = self
            .module
            .static_inits
            .iter()
            .map(|s| (s.class, s.field, s.value.clone()))
            .collect();
        for (class, field, value) in inits {
            let v = self.const_value(&value);
            self.statics.insert((class, field), v);
        }
        let elems: Vec<Value> = args.iter().cloned().map(Value::Str).collect();
        let args_h = self.heap.alloc(SlotData::Arr {
            elems,
            user: false,
        });
        self.globals = vec![Value::Null; self.module.global_count.max(1) as usize];
        self.globals[0] = Value::Arr(args_h);
        let entry = self.module.entry;
        if (entry as usize) < self.module.functions.len() {
            self.push_frame(entry, Vec::new(), None);
        } else {
            self.fail_plain("missing entry function");
        }
    }

    pub fn run(&mut self) {
        while self.exit.is_none() {
            self.step();
        }
    }

    pub fn step(&mut self) -> Status {
        if let Some(code) = self.exit {
            return Status::Exited(code);
        }
        let Some(frame) = self.frames.last() else {
            self.finish(0);
            return Status::Exited(0);
        };
        let func = frame.func;
        let ip = frame.ip as usize;
        let Some(op) = self.module.functions.get(func as usize).and_then(|f| f.code.get(ip).cloned()) else {
            self.do_return();
            return self.status();
        };
        self.exec(op);
        self.status()
    }

    pub fn finished(&self) -> bool {
        self.exit.is_some()
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn outcome(&self) -> Outcome {
        Outcome {
            exit: self.exit.unwrap_or(0),
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        }
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn location(&self) -> Option<FrameView> {
        let frame = self.frames.last()?;
        Some(self.view(frame))
    }

    pub fn backtrace(&self) -> Vec<FrameView> {
        self.frames.iter().rev().map(|f| self.view(f)).collect()
    }

    pub fn locals(&self) -> Vec<(String, String)> {
        let Some(frame) = self.frames.last() else {
            return Vec::new();
        };
        let names = self
            .module
            .functions
            .get(frame.func as usize)
            .map(|f| f.local_names.as_slice())
            .unwrap_or(&[]);
        names
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.is_empty())
            .map(|(i, n)| {
                let shown = frame
                    .locals
                    .get(i)
                    .map(|v| self.show(v, 0))
                    .unwrap_or_else(|| "null".into());
                (n.clone(), shown)
            })
            .collect()
    }

    pub fn lookup_local(&self, name: &str) -> Option<String> {
        self.locals()
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    fn status(&self) -> Status {
        match self.exit {
            Some(code) => Status::Exited(code),
            None => Status::Running,
        }
    }

    fn view(&self, frame: &Frame) -> FrameView {
        let func = self
            .module
            .functions
            .get(frame.func as usize)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "?".into());
        let loc = self
            .module
            .functions
            .get(frame.func as usize)
            .and_then(|f| f.locs.get(frame.ip as usize).copied());
        let (file, line, col) = if let Some(loc) = loc {
            let name = self
                .module
                .files
                .get(loc.file as usize)
                .cloned()
                .unwrap_or_default();
            (name, loc.line, loc.col)
        } else {
            (String::new(), 0, 0)
        };
        FrameView { func, file, line, col }
    }

    fn exec(&mut self, op: Op) {
        let result = self.exec_op(op);
        if let Err(fault) = result {
            self.raise(fault);
        }
    }

    fn exec_op(&mut self, op: Op) -> Result<(), Fault> {
        match op {
            Op::Const(id) => {
                let c = self
                    .module
                    .constants
                    .get(id as usize)
                    .cloned()
                    .ok_or_else(|| Fault::Type("bad constant".into()))?;
                let v = self.const_value(&c);
                self.push(v);
                self.bump();
            }
            Op::Load(slot) => {
                let v = self
                    .frames
                    .last()
                    .and_then(|f| f.locals.get(slot as usize).cloned())
                    .unwrap_or(Value::Null);
                self.push(v);
                self.bump();
            }
            Op::Store(slot) => {
                let v = self.pop()?;
                self.store_local(slot, v)?;
                self.bump();
            }
            Op::LoadField(index) => {
                let recv = self.pop()?;
                let v = self.read_field(&recv, index)?;
                self.push(v);
                self.bump();
            }
            Op::StoreField(index) => {
                let value = self.pop()?;
                let recv = self.pop()?;
                self.write_field(&recv, index, value)?;
                self.bump();
            }
            Op::LoadStatic(class, index) => {
                let v = self
                    .statics
                    .get(&(class, index))
                    .cloned()
                    .unwrap_or(Value::Null);
                self.push(v);
                self.bump();
            }
            Op::StoreStatic(class, index) => {
                let v = self.pop()?;
                self.statics.insert((class, index), v);
                self.bump();
            }
            Op::LoadGlobal(index) => {
                let v = self
                    .globals
                    .get(index as usize)
                    .cloned()
                    .unwrap_or(Value::Null);
                self.push(v);
                self.bump();
            }
            Op::Pop => {
                self.pop()?;
                self.bump();
            }
            Op::Dup => {
                let v = self.peek()?.clone();
                self.push(v);
                self.bump();
            }
            Op::Add => self.arith(Arith::Add)?,
            Op::Sub => self.arith(Arith::Sub)?,
            Op::Mul => self.arith(Arith::Mul)?,
            Op::Div => self.arith(Arith::Div)?,
            Op::Mod => self.arith(Arith::Mod)?,
            Op::Neg => {
                let v = self.pop()?;
                self.push(self.neg(v)?);
                self.bump();
            }
            Op::Eq => self.cmp(Cmp::Eq)?,
            Op::Ne => self.cmp(Cmp::Ne)?,
            Op::Lt => self.cmp(Cmp::Lt)?,
            Op::Le => self.cmp(Cmp::Le)?,
            Op::Gt => self.cmp(Cmp::Gt)?,
            Op::Ge => self.cmp(Cmp::Ge)?,
            Op::Not => {
                let v = self.pop()?;
                let Value::Bool(b) = v else {
                    return Err(Fault::Type("not expects bool".into()));
                };
                self.push(Value::Bool(!b));
                self.bump();
            }
            Op::Jump(ip) => self.jump(ip),
            Op::JumpIfFalse(ip) => {
                let v = self.pop()?;
                let Value::Bool(b) = v else {
                    return Err(Fault::Type("condition expects bool".into()));
                };
                if b {
                    self.bump();
                } else {
                    self.jump(ip);
                }
            }
            Op::Call(func) => {
                let arity = self.fn_arity(func)?;
                let args = self.pop_n(arity as usize)?;
                self.bump();
                self.push_frame(func, args, None);
            }
            Op::CallVirtual(slot, argc) => {
                let args = self.pop_n(argc as usize)?;
                let func = self.virtual_func(args.first(), slot)?;
                self.bump();
                self.push_frame(func, args, None);
            }
            Op::CallInterface(iface, slot, argc) => {
                let args = self.pop_n(argc as usize)?;
                let func = self.iface_func(args.first(), iface, slot)?;
                self.bump();
                self.push_frame(func, args, None);
            }
            Op::MakeTask(func) => {
                let arity = self.fn_arity(func)?;
                let args = self.pop_n(arity as usize)?;
                let returns_void = self.fn_void(func);
                let h = self.heap.alloc(SlotData::Task(Task {
                    kind: TaskKind::Direct(func),
                    args,
                    state: TaskState::Fresh,
                    result: None,
                    error: None,
                    returns_void,
                    slot: 0,
                }));
                self.push(Value::Task(h));
                self.bump();
            }
            Op::MakeTaskVirtual(slot, argc) => {
                let args = self.pop_n(argc as usize)?;
                let h = self.heap.alloc(SlotData::Task(Task {
                    kind: TaskKind::Virtual { slot },
                    args,
                    state: TaskState::Fresh,
                    result: None,
                    error: None,
                    returns_void: false,
                    slot,
                }));
                self.push(Value::Task(h));
                self.bump();
            }
            Op::MakeTaskInterface(iface, slot, argc) => {
                let args = self.pop_n(argc as usize)?;
                let h = self.heap.alloc(SlotData::Task(Task {
                    kind: TaskKind::Interface { iface, slot },
                    args,
                    state: TaskState::Fresh,
                    result: None,
                    error: None,
                    returns_void: false,
                    slot,
                }));
                self.push(Value::Task(h));
                self.bump();
            }
            Op::Return => self.do_return(),
            Op::New(class) => {
                let n = self
                    .module
                    .classes
                    .get(class as usize)
                    .map(|c| c.field_names.len())
                    .ok_or_else(|| Fault::Type("bad class".into()))?;
                let h = self.heap.alloc(SlotData::Obj {
                    class,
                    fields: vec![Value::Null; n],
                    user: true,
                });
                self.push(Value::Obj(h));
                self.bump();
            }
            Op::NewArray => {
                let n = as_int(self.pop()?)?;
                if n < 0 {
                    return Err(Fault::Index { len: 0, index: n });
                }
                let h = self.heap.alloc(SlotData::Arr {
                    elems: vec![Value::Null; n as usize],
                    user: true,
                });
                self.push(Value::Arr(h));
                self.bump();
            }
            Op::ArrayLit(n) => {
                let elems = self.pop_n(n as usize)?;
                let h = self.heap.alloc(SlotData::Arr {
                    elems,
                    user: true,
                });
                self.push(Value::Arr(h));
                self.bump();
            }
            Op::GetIndex => {
                let index = as_int(self.pop()?)?;
                let recv = self.pop()?;
                let v = self.index_get(&recv, index)?;
                self.push(v);
                self.bump();
            }
            Op::SetIndex => {
                let value = self.pop()?;
                let index = as_int(self.pop()?)?;
                let recv = self.pop()?;
                self.index_set(&recv, index, value)?;
                self.bump();
            }
            Op::Len => {
                let v = self.pop()?;
                let n = self.len_of(&v)?;
                self.push(Value::Int(n));
                self.bump();
            }
            Op::Throw => {
                let v = self.pop()?;
                self.throw_value(v);
            }
            Op::PushHandler(ip) => {
                if let Some(frame) = self.frames.last_mut() {
                    frame.handlers.push(Handler { ip });
                    frame.ip += 1;
                }
            }
            Op::PopHandler => {
                if let Some(frame) = self.frames.last_mut() {
                    frame.handlers.pop();
                    frame.ip += 1;
                }
            }
            Op::IsType(class) => {
                let v = self.pop()?;
                self.push(Value::Bool(self.is_type(&v, class)));
                self.bump();
            }
            Op::Delete => {
                let v = self.pop()?;
                self.delete_value(&v)?;
                self.bump();
            }
            Op::Await => self.await_task()?,
            Op::Print => {
                let v = self.pop()?;
                let s = self.show(&v, 0);
                self.write_out(&s);
                self.bump();
            }
            Op::PrintLn => {
                let v = self.pop()?;
                let s = self.show(&v, 0);
                self.write_out(&s);
                self.write_out("\n");
                self.bump();
            }
            Op::ToString => {
                let v = self.pop()?;
                let s = self.show(&v, 0);
                self.push(Value::Str(s));
                self.bump();
            }
            Op::ToInt => {
                let v = self.pop()?;
                let n = self.to_int(&v)?;
                self.push(Value::Int(n));
                self.bump();
            }
            Op::ToFloat => {
                let v = self.pop()?;
                let n = self.to_float(&v)?;
                self.push(Value::Float(n));
                self.bump();
            }
            Op::ToDouble => {
                let v = self.pop()?;
                let n = self.to_double(&v)?;
                self.push(Value::Double(n));
                self.bump();
            }
            Op::Range => {
                let hi = as_int(self.pop()?)?;
                let lo = as_int(self.pop()?)?;
                self.push(Value::Range { lo, hi });
                self.bump();
            }
            Op::RangeStart => {
                let v = self.pop()?;
                let Value::Range { lo, .. } = v else {
                    return Err(Fault::Type("range expected".into()));
                };
                self.push(Value::Int(lo));
                self.bump();
            }
            Op::RangeEnd => {
                let v = self.pop()?;
                let Value::Range { hi, .. } = v else {
                    return Err(Fault::Type("range expected".into()));
                };
                self.push(Value::Int(hi));
                self.bump();
            }
            Op::Host(id) => {
                hostcall::exec(self, id)?;
                self.bump();
            }
        }
        Ok(())
    }

    fn await_task(&mut self) -> Result<(), Fault> {
        let v = self.pop()?;
        let Value::Task(h) = v else {
            return Err(Fault::Type("await expects a task".into()));
        };
        let (state, result, error, returns_void) = {
            let slot = self.heap.slot(h)?;
            let SlotData::Task(t) = &slot.data else {
                return Err(Fault::Type("await expects a task".into()));
            };
            (t.state, t.result.clone(), t.error.clone(), t.returns_void)
        };
        match state {
            TaskState::Done => {
                if !returns_void {
                    self.push(result.unwrap_or(Value::Null));
                }
                self.bump();
            }
            TaskState::Failed => {
                self.throw_value(error.unwrap_or(Value::Null));
            }
            TaskState::Running => {
                return Err(Fault::Type("task is already running".into()));
            }
            TaskState::Fresh => {
                self.push(Value::Task(h));
                self.start_task(h)?;
            }
        }
        Ok(())
    }

    fn start_task(&mut self, h: Handle) -> Result<(), Fault> {
        let (kind, args, slot) = {
            let slot_ref = self.heap.slot(h)?;
            let SlotData::Task(t) = &slot_ref.data else {
                return Err(Fault::Type("not a task".into()));
            };
            (t.kind.clone(), t.args.clone(), t.slot)
        };
        let (func, returns_void) = match kind {
            TaskKind::Direct(func) => (func, self.fn_void(func)),
            TaskKind::Virtual { slot } => {
                let func = self.virtual_func(args.first(), slot)?;
                (func, self.fn_void(func))
            }
            TaskKind::Interface { iface, slot } => {
                let func = self.iface_func(args.first(), iface, slot)?;
                (func, self.fn_void(func))
            }
        };
        let _ = slot;
        if let Ok(slot) = self.heap.slot_mut(h) {
            if let SlotData::Task(t) = &mut slot.data {
                t.state = TaskState::Running;
                t.returns_void = returns_void;
            }
        }
        self.push_frame(func, args, Some(h));
        Ok(())
    }

    fn arith(&mut self, op: Arith) -> Result<(), Fault> {
        let r = self.pop()?;
        let l = self.pop()?;
        let v = match (&l, &r) {
            (Value::Int(a), Value::Int(b)) => Value::Int(self.int_op(op, *a, *b)?),
            (Value::Float(a), Value::Float(b)) => Value::Float(self.float_op(op, *a, *b)?),
            (Value::Double(a), Value::Double(b)) => Value::Double(self.double_op(op, *a, *b)?),
            (Value::Str(a), Value::Str(b)) if matches!(op, Arith::Add) => {
                Value::Str(format!("{a}{b}"))
            }
            _ => return Err(Fault::Type("numeric operands expected".into())),
        };
        self.push(v);
        self.bump();
        Ok(())
    }

    fn int_op(&self, op: Arith, a: i64, b: i64) -> Result<i64, Fault> {
        let v = match op {
            Arith::Add => a.checked_add(b),
            Arith::Sub => a.checked_sub(b),
            Arith::Mul => a.checked_mul(b),
            Arith::Div => {
                if b == 0 {
                    return Err(Fault::DivZero);
                }
                a.checked_div(b)
            }
            Arith::Mod => {
                if b == 0 {
                    return Err(Fault::DivZero);
                }
                a.checked_rem(b)
            }
        };
        v.ok_or(Fault::Overflow)
    }

    fn float_op(&self, op: Arith, a: f32, b: f32) -> Result<f32, Fault> {
        if matches!(op, Arith::Div | Arith::Mod) && b == 0.0 {
            return Err(Fault::DivZero);
        }
        Ok(match op {
            Arith::Add => a + b,
            Arith::Sub => a - b,
            Arith::Mul => a * b,
            Arith::Div => a / b,
            Arith::Mod => a % b,
        })
    }

    fn double_op(&self, op: Arith, a: f64, b: f64) -> Result<f64, Fault> {
        if matches!(op, Arith::Div | Arith::Mod) && b == 0.0 {
            return Err(Fault::DivZero);
        }
        Ok(match op {
            Arith::Add => a + b,
            Arith::Sub => a - b,
            Arith::Mul => a * b,
            Arith::Div => a / b,
            Arith::Mod => a % b,
        })
    }

    fn neg(&self, v: Value) -> Result<Value, Fault> {
        match v {
            Value::Int(n) => n.checked_neg().map(Value::Int).ok_or(Fault::Overflow),
            Value::Float(n) => Ok(Value::Float(-n)),
            Value::Double(n) => Ok(Value::Double(-n)),
            _ => Err(Fault::Type("neg expects a number".into())),
        }
    }

    fn cmp(&mut self, op: Cmp) -> Result<(), Fault> {
        let r = self.pop()?;
        let l = self.pop()?;
        let eq = values_eq(&l, &r);
        let ord = num_ord(&l, &r);
        let bit = match op {
            Cmp::Eq => eq,
            Cmp::Ne => !eq,
            Cmp::Lt | Cmp::Le | Cmp::Gt | Cmp::Ge => {
                let Some(o) = ord else {
                    return Err(Fault::Type("comparison expects numbers".into()));
                };
                match op {
                    Cmp::Lt => o.is_lt(),
                    Cmp::Le => o.is_le(),
                    Cmp::Gt => o.is_gt(),
                    Cmp::Ge => o.is_ge(),
                    _ => false,
                }
            }
        };
        self.push(Value::Bool(bit));
        self.bump();
        Ok(())
    }

    fn do_return(&mut self) {
        let Some(frame) = self.frames.pop() else {
            self.finish(0);
            return;
        };
        let returns_void = self
            .module
            .functions
            .get(frame.func as usize)
            .map(|f| f.returns_void)
            .unwrap_or(true);
        let value = if returns_void {
            None
        } else {
            frame.stack.last().cloned()
        };
        if let Some(task) = frame.task {
            if let Ok(slot) = self.heap.slot_mut(task) {
                if let SlotData::Task(t) = &mut slot.data {
                    t.state = TaskState::Done;
                    t.result = value;
                }
            }
            return;
        }
        self.last_value = value.clone();
        if self.frames.is_empty() {
            let code = match value {
                Some(Value::Int(n)) => n as i32,
                _ => 0,
            };
            self.finish(code);
        } else if let Some(v) = value {
            self.frames.last_mut().unwrap().stack.push(v);
        }
    }

    fn raise(&mut self, fault: Fault) {
        let (class, msg) = match &fault {
            Fault::DivZero => ("DivZero", rt(self.lang, RtMsg::DivZero)),
            Fault::NullRef => ("NullRef", rt(self.lang, RtMsg::NullRef)),
            Fault::UseAfterFree => ("UseAfterFree", rt(self.lang, RtMsg::UseAfterFree)),
            Fault::DoubleFree => ("UseAfterFree", rt(self.lang, RtMsg::DoubleFree)),
            Fault::Index { len, index } => (
                "IndexError",
                rt(self.lang, RtMsg::Index { len: *len, index: *index }),
            ),
            Fault::Overflow => ("Overflow", rt(self.lang, RtMsg::Overflow)),
            Fault::Type(detail) => (
                "TypeError",
                rt(self.lang, RtMsg::Type { detail: detail.clone() }),
            ),
            Fault::Cast(detail) => (
                "CastError",
                rt(self.lang, RtMsg::Cast { detail: detail.clone() }),
            ),
            Fault::Io(detail) => ("IoError", detail.clone()),
        };
        match self.make_exception(class, msg) {
            Some(v) => self.throw_value(v),
            None => self.fail_plain(&rt(
                self.lang,
                RtMsg::Type {
                    detail: class.into(),
                },
            )),
        }
    }

    fn make_exception(&mut self, name: &str, message: String) -> Option<Value> {
        let id = self
            .module
            .classes
            .iter()
            .position(|c| c.name == name)? as u32;
        let n = self.module.classes[id as usize].field_names.len();
        let mut fields = vec![Value::Null; n];
        if !fields.is_empty() {
            fields[0] = Value::Str(message);
        }
        let h = self.heap.alloc(SlotData::Obj {
            class: id,
            fields,
            user: false,
        });
        Some(Value::Obj(h))
    }

    fn throw_value(&mut self, value: Value) {
        let trace = self.trace_text();
        while let Some(frame) = self.frames.last_mut() {
            if let Some(handler) = frame.handlers.pop() {
                frame.stack.clear();
                frame.stack.push(value);
                frame.ip = handler.ip;
                return;
            }
            let task = frame.task;
            self.frames.pop();
            if let Some(task) = task {
                if let Ok(slot) = self.heap.slot_mut(task) {
                    if let SlotData::Task(t) = &mut slot.data {
                        t.state = TaskState::Failed;
                        t.error = Some(value);
                    }
                }
                return;
            }
        }
        self.uncaught(&value, &trace);
    }

    fn uncaught(&mut self, value: &Value, trace: &str) {
        let header = rt(self.lang, RtMsg::UncaughtHeader);
        let text = self.exception_text(value);
        self.write_err(&format!("{header}\n{text}\n{trace}"));
        self.finish(1);
    }

    fn exception_text(&self, value: &Value) -> String {
        match value {
            Value::Obj(h) => {
                let class = self
                    .heap
                    .slot(*h)
                    .ok()
                    .and_then(|s| match &s.data {
                        SlotData::Obj { class, fields, .. } => {
                            let name = self
                                .module
                                .classes
                                .get(*class as usize)
                                .map(|c| c.name.as_str())
                                .unwrap_or("Exception");
                            let msg = fields.first().map(|v| self.show(v, 0)).unwrap_or_default();
                            Some(format!("{name}: {msg}"))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "Exception".into());
                class
            }
            other => self.show(other, 0),
        }
    }

    fn trace_text(&self) -> String {
        let at = rt(self.lang, RtMsg::At);
        let mut lines = Vec::new();
        for view in self.backtrace() {
            if view.func == "$entry" || view.line == 0 {
                continue;
            }
            lines.push(format!("{at} {} ({}:{})", view.func, view.file, view.line));
        }
        lines.join("\n")
    }

    fn is_type(&self, value: &Value, class: u32) -> bool {
        let Value::Obj(h) = value else {
            return false;
        };
        let Ok(slot) = self.heap.slot(*h) else {
            return false;
        };
        let SlotData::Obj { class: start, .. } = &slot.data else {
            return false;
        };
        let mut cur = Some(*start);
        while let Some(id) = cur {
            if id == class {
                return true;
            }
            cur = self.module.classes.get(id as usize).and_then(|c| c.super_id);
        }
        false
    }

    fn read_field(&self, recv: &Value, index: u16) -> Result<Value, Fault> {
        let h = self.obj_handle(recv)?;
        let slot = self.heap.slot(h)?;
        match &slot.data {
            SlotData::Obj { fields, .. } => fields
                .get(index as usize)
                .cloned()
                .ok_or_else(|| Fault::Type("bad field".into())),
            _ => Err(Fault::Type("not an object".into())),
        }
    }

    fn write_field(&mut self, recv: &Value, index: u16, value: Value) -> Result<(), Fault> {
        let h = self.obj_handle(recv)?;
        let slot = self.heap.slot_mut(h)?;
        match &mut slot.data {
            SlotData::Obj { fields, .. } => {
                let slot = fields
                    .get_mut(index as usize)
                    .ok_or_else(|| Fault::Type("bad field".into()))?;
                *slot = value;
                Ok(())
            }
            _ => Err(Fault::Type("not an object".into())),
        }
    }

    fn index_get(&self, recv: &Value, index: i64) -> Result<Value, Fault> {
        match recv {
            Value::Null => Err(Fault::NullRef),
            Value::Arr(h) => {
                let slot = self.heap.slot(*h)?;
                match &slot.data {
                    SlotData::Arr { elems, .. } => {
                        if index < 0 || index as usize >= elems.len() {
                            Err(Fault::Index {
                                len: elems.len() as i64,
                                index,
                            })
                        } else {
                            Ok(elems[index as usize].clone())
                        }
                    }
                    _ => Err(Fault::Type("not an array".into())),
                }
            }
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                if index < 0 || index as usize >= chars.len() {
                    Err(Fault::Index {
                        len: chars.len() as i64,
                        index,
                    })
                } else {
                    Ok(Value::Str(chars[index as usize].to_string()))
                }
            }
            _ => Err(Fault::Type("not an array".into())),
        }
    }

    fn index_set(&mut self, recv: &Value, index: i64, value: Value) -> Result<(), Fault> {
        let Value::Arr(h) = recv else {
            return if matches!(recv, Value::Null) {
                Err(Fault::NullRef)
            } else {
                Err(Fault::Type("not an array".into()))
            };
        };
        let slot = self.heap.slot_mut(*h)?;
        match &mut slot.data {
            SlotData::Arr { elems, .. } => {
                if index < 0 || index as usize >= elems.len() {
                    Err(Fault::Index {
                        len: elems.len() as i64,
                        index,
                    })
                } else {
                    elems[index as usize] = value;
                    Ok(())
                }
            }
            _ => Err(Fault::Type("not an array".into())),
        }
    }

    fn len_of(&self, v: &Value) -> Result<i64, Fault> {
        match v {
            Value::Null => Err(Fault::NullRef),
            Value::Str(s) => Ok(s.chars().count() as i64),
            Value::Arr(h) => {
                let slot = self.heap.slot(*h)?;
                match &slot.data {
                    SlotData::Arr { elems, .. } => Ok(elems.len() as i64),
                    _ => Err(Fault::Type("len expects an array or string".into())),
                }
            }
            _ => Err(Fault::Type("len expects an array or string".into())),
        }
    }

    fn delete_value(&mut self, v: &Value) -> Result<(), Fault> {
        match v {
            Value::Null => Err(Fault::NullRef),
            Value::Obj(h) | Value::Arr(h) | Value::Host(h) => self.heap.free(*h),
            Value::Task(_) => Err(Fault::Type("cannot delete a task".into())),
            _ => Err(Fault::Type("delete expects an object or array".into())),
        }
    }

    fn obj_handle(&self, v: &Value) -> Result<Handle, Fault> {
        match v {
            Value::Null => Err(Fault::NullRef),
            Value::Obj(h) => {
                self.heap.slot(*h)?;
                Ok(*h)
            }
            _ => Err(Fault::Type("not an object".into())),
        }
    }

    fn virtual_func(&self, recv: Option<&Value>, slot: u16) -> Result<u32, Fault> {
        let class = self.class_of(recv.ok_or_else(|| Fault::Type("missing receiver".into()))?)?;
        self.module
            .classes
            .get(class as usize)
            .and_then(|c| c.vtable.get(slot as usize).copied())
            .ok_or_else(|| Fault::Type("missing virtual method".into()))
    }

    fn iface_func(&self, recv: Option<&Value>, iface: u32, slot: u16) -> Result<u32, Fault> {
        let class = self.class_of(recv.ok_or_else(|| Fault::Type("missing receiver".into()))?)?;
        self.module
            .classes
            .get(class as usize)
            .and_then(|c| c.itables.iter().find(|t| t.iface == iface))
            .and_then(|t| t.methods.get(slot as usize).copied())
            .ok_or_else(|| Fault::Type("missing interface method".into()))
    }

    fn class_of(&self, v: &Value) -> Result<u32, Fault> {
        match v {
            Value::Null => Err(Fault::NullRef),
            Value::Obj(h) => {
                let slot = self.heap.slot(*h)?;
                match &slot.data {
                    SlotData::Obj { class, .. } => Ok(*class),
                    _ => Err(Fault::Type("not an object".into())),
                }
            }
            _ => Err(Fault::Type("not an object".into())),
        }
    }

    fn push_frame(&mut self, func: u32, args: Vec<Value>, task: Option<Handle>) {
        let (locals_n, _arity) = self
            .module
            .functions
            .get(func as usize)
            .map(|f| (f.locals as usize, f.arity))
            .unwrap_or((0, 0));
        let mut locals = vec![Value::Null; locals_n.max(args.len())];
        for (i, arg) in args.into_iter().enumerate() {
            if i < locals.len() {
                locals[i] = arg;
            }
        }
        self.frames.push(Frame {
            func,
            ip: 0,
            locals,
            stack: Vec::new(),
            handlers: Vec::new(),
            task,
        });
    }

    fn fn_arity(&self, func: u32) -> Result<u16, Fault> {
        self.module
            .functions
            .get(func as usize)
            .map(|f| f.arity)
            .ok_or_else(|| Fault::Type("bad function".into()))
    }

    fn fn_void(&self, func: u32) -> bool {
        self.module
            .functions
            .get(func as usize)
            .map(|f| f.returns_void)
            .unwrap_or(true)
    }

    fn store_local(&mut self, slot: u16, value: Value) -> Result<(), Fault> {
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| Fault::Type("no frame".into()))?;
        if frame.locals.len() <= slot as usize {
            frame.locals.resize(slot as usize + 1, Value::Null);
        }
        frame.locals[slot as usize] = value;
        Ok(())
    }

    pub(crate) fn push(&mut self, v: Value) {
        if let Some(frame) = self.frames.last_mut() {
            frame.stack.push(v);
        }
    }

    pub(crate) fn pop(&mut self) -> Result<Value, Fault> {
        self.frames
            .last_mut()
            .and_then(|f| f.stack.pop())
            .ok_or_else(|| Fault::Type("stack underflow".into()))
    }

    pub(crate) fn pop_n(&mut self, n: usize) -> Result<Vec<Value>, Fault> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.pop()?);
        }
        v.reverse();
        Ok(v)
    }

    fn peek(&self) -> Result<&Value, Fault> {
        self.frames
            .last()
            .and_then(|f| f.stack.last())
            .ok_or_else(|| Fault::Type("stack underflow".into()))
    }

    fn bump(&mut self) {
        if let Some(frame) = self.frames.last_mut() {
            frame.ip += 1;
        }
    }

    fn jump(&mut self, ip: u32) {
        if let Some(frame) = self.frames.last_mut() {
            frame.ip = ip;
        }
    }

    fn const_value(&self, c: &Constant) -> Value {
        match c {
            Constant::Int(n) => Value::Int(*n),
            Constant::Float(n) => Value::Float(*n),
            Constant::Double(n) => Value::Double(*n),
            Constant::Bool(b) => Value::Bool(*b),
            Constant::Str(s) => Value::Str(s.clone()),
            Constant::Null => Value::Null,
            Constant::Enum { id, variant } => Value::Enum {
                id: *id,
                variant: *variant,
            },
        }
    }

    fn to_int(&self, v: &Value) -> Result<i64, Fault> {
        match v {
            Value::Int(n) => Ok(*n),
            Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
            Value::Float(n) => float_to_int(*n as f64),
            Value::Double(n) => float_to_int(*n),
            Value::Str(s) => s.trim().parse::<i64>().map_err(|_| {
                Fault::Cast(format!("'{s}' is not an int"))
            }),
            _ => Err(Fault::Cast("value is not an int".into())),
        }
    }

    fn to_float(&self, v: &Value) -> Result<f32, Fault> {
        match v {
            Value::Float(n) => Ok(*n),
            Value::Int(n) => Ok(*n as f32),
            Value::Double(n) => Ok(*n as f32),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Str(s) => s.trim().parse::<f32>().map_err(|_| {
                Fault::Cast(format!("'{s}' is not a float"))
            }),
            _ => Err(Fault::Cast("value is not a float".into())),
        }
    }

    fn to_double(&self, v: &Value) -> Result<f64, Fault> {
        match v {
            Value::Double(n) => Ok(*n),
            Value::Float(n) => Ok(*n as f64),
            Value::Int(n) => Ok(*n as f64),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Str(s) => s.trim().parse::<f64>().map_err(|_| {
                Fault::Cast(format!("'{s}' is not a double"))
            }),
            _ => Err(Fault::Cast("value is not a double".into())),
        }
    }

    fn show(&self, v: &Value, depth: u32) -> String {
        if depth > 4 {
            return "...".into();
        }
        match v {
            Value::Int(n) => n.to_string(),
            Value::Float(n) => format_float(*n as f64),
            Value::Double(n) => format_float(*n),
            Value::Bool(b) => if *b { "true" } else { "false" }.into(),
            Value::Str(s) => s.clone(),
            Value::Null => "null".into(),
            Value::Obj(h) => {
                let name = self
                    .heap
                    .slot(*h)
                    .ok()
                    .and_then(|s| match &s.data {
                        SlotData::Obj { class, .. } => self
                            .module
                            .classes
                            .get(*class as usize)
                            .map(|c| c.name.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "Object".into());
                format!("{name}@{}", h.index)
            }
            Value::Arr(h) => {
                let elems = self
                    .heap
                    .slot(*h)
                    .ok()
                    .and_then(|s| match &s.data {
                        SlotData::Arr { elems, .. } => Some(elems.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let parts: Vec<String> = elems.iter().map(|e| self.show(e, depth + 1)).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Task(_) => "task".into(),
            Value::Enum { id, variant } => {
                let en = self.module.enums.get(*id as usize);
                let name = en.map(|e| e.name.as_str()).unwrap_or("enum");
                let var = en
                    .and_then(|e| e.variants.get(*variant as usize))
                    .map(|s| s.as_str())
                    .unwrap_or("?");
                format!("{name}.{var}")
            }
            Value::Range { lo, hi } => format!("{lo}..{hi}"),
            Value::Host(h) => {
                let kind = self
                    .heap
                    .slot(*h)
                    .ok()
                    .and_then(|s| match &s.data {
                        SlotData::Host(obj) => Some(obj.kind()),
                        _ => None,
                    })
                    .unwrap_or("host");
                format!("{kind}@{}", h.index)
            }
        }
    }

    fn write_out(&mut self, s: &str) {
        self.stdout.push_str(s);
        if self.echo {
            print!("{s}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }

    fn write_err(&mut self, s: &str) {
        if !self.stderr.is_empty() && !self.stderr.ends_with('\n') {
            self.stderr.push('\n');
        }
        self.stderr.push_str(s);
        if !s.ends_with('\n') {
            self.stderr.push('\n');
        }
        if self.echo {
            eprint!("{s}");
            if !s.ends_with('\n') {
                eprintln!();
            }
        }
    }

    fn fail_plain(&mut self, msg: &str) {
        self.write_err(msg);
        self.finish(1);
    }

    fn finish(&mut self, code: i32) {
        if self.exit.is_some() {
            return;
        }
        self.exit = Some(code);
        self.report_leaks();
    }

    fn report_leaks(&mut self) {
        if self.leaks_reported {
            return;
        }
        self.leaks_reported = true;
        let count = self.heap.user_live();
        if count > 0 {
            let msg = rt(self.lang, RtMsg::Leak { count });
            self.write_err(&msg);
        }
    }
}

#[derive(Clone, Copy)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Clone, Copy)]
enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Double(x), Value::Double(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::Obj(x), Value::Obj(y)) => x == y,
        (Value::Arr(x), Value::Arr(y)) => x == y,
        (Value::Task(x), Value::Task(y)) => x == y,
        (Value::Enum { id: a, variant: av }, Value::Enum { id: b, variant: bv }) => a == b && av == bv,
        (Value::Range { lo: a, hi: ah }, Value::Range { lo: b, hi: bh }) => a == b && ah == bh,
        (Value::Host(a), Value::Host(b)) => a == b,
        _ => false,
    }
}

fn num_ord(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Double(x), Value::Double(y)) => x.partial_cmp(y),
        _ => None,
    }
}

fn as_int(v: Value) -> Result<i64, Fault> {
    match v {
        Value::Int(n) => Ok(n),
        _ => Err(Fault::Type("int expected".into())),
    }
}

fn float_to_int(n: f64) -> Result<i64, Fault> {
    if !n.is_finite() || n < i64::MIN as f64 || n > i64::MAX as f64 {
        Err(Fault::Cast("number is out of int range".into()))
    } else {
        Ok(n as i64)
    }
}

fn format_float(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() {
        format!("{n:.1}")
    } else {
        n.to_string()
    }
}
