use crate::{DiagKind, Lang};

pub fn describe(lang: Lang, kind: &DiagKind) -> (&'static str, String) {
    let ru = lang == Lang::Ru;
    match kind {
        DiagKind::ExpectedSemicolon => pair(
            "E001",
            ru,
            "ожидался ';'",
            "expected ';'",
        ),
        DiagKind::UnexpectedEof => pair("E002", ru, "неожиданный конец файла", "unexpected end of file"),
        DiagKind::UnexpectedToken { found, expected } => (
            "E003",
            if ru {
                format!("неожиданный токен '{found}', ожидалось {expected}")
            } else {
                format!("unexpected token '{found}', expected {expected}")
            },
        ),
        DiagKind::UnterminatedString => pair(
            "E004",
            ru,
            "незакрытая строка",
            "unterminated string",
        ),
        DiagKind::UnterminatedComment => pair(
            "E005",
            ru,
            "незакрытый комментарий",
            "unterminated comment",
        ),
        DiagKind::InvalidNumber { literal } => (
            "E006",
            if ru {
                format!("некорректное число '{literal}'")
            } else {
                format!("invalid number '{literal}'")
            },
        ),
        DiagKind::InvalidEscape => pair("E007", ru, "некорректная escape-последовательность", "invalid escape"),
        DiagKind::ExpectedIdent => pair("E008", ru, "ожидался идентификатор", "expected identifier"),
        DiagKind::UndefinedName { name } => (
            "E009",
            if ru {
                format!("неизвестное имя '{name}'")
            } else {
                format!("undefined name '{name}'")
            },
        ),
        DiagKind::Duplicate { name } => (
            "E010",
            if ru {
                format!("повторное определение '{name}'")
            } else {
                format!("duplicate definition '{name}'")
            },
        ),
        DiagKind::TypeMismatch { expected, found } => (
            "E011",
            if ru {
                format!("несовпадение типов: ожидался {expected}, найден {found}")
            } else {
                format!("type mismatch: expected {expected}, found {found}")
            },
        ),
        DiagKind::MissingMain { hint } => (
            "E012",
            if ru {
                format!("нет точки входа main(args: string[]){hint}")
            } else {
                format!("missing entry point main(args: string[]){hint}")
            },
        ),
        DiagKind::MultipleMain => pair(
            "E013",
            ru,
            "несколько точек входа main",
            "multiple main entry points",
        ),
        DiagKind::Immutable { name } => (
            "E014",
            if ru {
                format!("нельзя изменить неизменяемое имя '{name}'")
            } else {
                format!("cannot assign to immutable name '{name}'")
            },
        ),
        DiagKind::NeedBool => pair(
            "E015",
            ru,
            "условие должно иметь тип bool",
            "condition must be bool",
        ),
        DiagKind::BadArity { name, expected, found } => (
            "E016",
            if ru {
                format!("'{name}': ожидалось аргументов {expected}, передано {found}")
            } else {
                format!("'{name}': expected {expected} arguments, found {found}")
            },
        ),
        DiagKind::Private { name } => (
            "E017",
            if ru {
                format!("'{name}' недоступно в этом контексте")
            } else {
                format!("'{name}' is not visible here")
            },
        ),
        DiagKind::NeedOverride { name } => (
            "E018",
            if ru {
                format!("метод '{name}' переопределяет метод родителя и требует override")
            } else {
                format!("method '{name}' overrides a parent method and needs override")
            },
        ),
        DiagKind::NoOverride { name } => (
            "E019",
            if ru {
                format!("override: метод '{name}' ничего не переопределяет")
            } else {
                format!("override: method '{name}' does not override anything")
            },
        ),
        DiagKind::FinalMethod { name } => (
            "E020",
            if ru {
                format!("нельзя переопределить final-метод '{name}'")
            } else {
                format!("cannot override final method '{name}'")
            },
        ),
        DiagKind::FinalClass { name } => (
            "E021",
            if ru {
                format!("нельзя наследовать final-класс '{name}'")
            } else {
                format!("cannot extend final class '{name}'")
            },
        ),
        DiagKind::MissingImpl { class, method } => (
            "E022",
            if ru {
                format!("класс '{class}' не реализует '{method}'")
            } else {
                format!("class '{class}' does not implement '{method}'")
            },
        ),
        DiagKind::NotThrowable { name } => (
            "E023",
            if ru {
                format!("'{name}' не является исключением")
            } else {
                format!("'{name}' is not an exception type")
            },
        ),
        DiagKind::NeedConst => pair(
            "E024",
            ru,
            "инициализатор const должен быть константным выражением",
            "const initializer must be a constant expression",
        ),
        DiagKind::Cycle { name } => (
            "E025",
            if ru {
                format!("цикл в наследовании '{name}'")
            } else {
                format!("inheritance cycle involving '{name}'")
            },
        ),
        DiagKind::NoField { name } => (
            "E026",
            if ru {
                format!("нет поля '{name}'")
            } else {
                format!("no field '{name}'")
            },
        ),
        DiagKind::NoMethod { name } => (
            "E027",
            if ru {
                format!("нет метода '{name}'")
            } else {
                format!("no method '{name}'")
            },
        ),
        DiagKind::BadIter => pair(
            "E028",
            ru,
            "for-in ожидает массив или диапазон",
            "for-in expects an array or a range",
        ),
        DiagKind::PackageMismatch { declared, expected } => (
            "E029",
            if ru {
                format!("пакет '{declared}' не совпадает с путём '{expected}'")
            } else {
                format!("package '{declared}' does not match path '{expected}'")
            },
        ),
        DiagKind::CannotNew { name } => (
            "E030",
            if ru {
                format!("нельзя создать экземпляр '{name}'")
            } else {
                format!("cannot instantiate '{name}'")
            },
        ),
        DiagKind::SuperFirst => pair(
            "E031",
            ru,
            "вызов super() должен быть первым оператором конструктора",
            "super() must be the first statement in a constructor",
        ),
        DiagKind::NeedCtor { parent } => (
            "E032",
            if ru {
                format!("нужен конструктор: родитель '{parent}' требует аргументы")
            } else {
                format!("constructor required: parent '{parent}' takes arguments")
            },
        ),
        DiagKind::BadModifier { detail } => ("E033", detail.clone()),
        DiagKind::NotTask => pair(
            "E034",
            ru,
            "await ожидает значение task",
            "await expects a task",
        ),
        DiagKind::NeedIntIndex => pair(
            "E035",
            ru,
            "индекс массива должен иметь тип int",
            "array index must be int",
        ),
        DiagKind::UnknownImport { path } => (
            "E036",
            if ru {
                format!("неизвестный импорт '{path}'")
            } else {
                format!("unknown import '{path}'")
            },
        ),
        DiagKind::CtorReturn => pair(
            "E037",
            ru,
            "конструктор не возвращает значение",
            "constructor cannot return a value",
        ),
        DiagKind::BadEntry { detail } => ("E038", detail.clone()),
        DiagKind::NotInLoop { what } => (
            "E039",
            if ru {
                format!("'{what}' вне цикла")
            } else {
                format!("'{what}' outside of a loop")
            },
        ),
        DiagKind::UnknownType { name } => (
            "E040",
            if ru {
                format!("неизвестный тип '{name}'")
            } else {
                format!("unknown type '{name}'")
            },
        ),
        DiagKind::NotCallable { name } => (
            "E041",
            if ru {
                format!("'{name}' нельзя вызвать")
            } else {
                format!("'{name}' is not callable")
            },
        ),
        DiagKind::MissingReturn { name } => (
            "E042",
            if ru {
                format!("'{name}' не всегда возвращает значение")
            } else {
                format!("'{name}' does not return on every path")
            },
        ),
        DiagKind::BadAssign => pair(
            "E043",
            ru,
            "левая часть присваивания недопустима",
            "invalid assignment target",
        ),
        DiagKind::InterfaceMember => pair(
            "E044",
            ru,
            "интерфейс может содержать только сигнатуры методов",
            "an interface can only declare method signatures",
        ),
        DiagKind::MissingBody { name } => (
            "E045",
            if ru {
                format!("у '{name}' нет тела")
            } else {
                format!("'{name}' is missing a body")
            },
        ),
        DiagKind::ProjectMissing => pair(
            "E046",
            ru,
            "проект Terra не инициализирован в этом каталоге. Сначала выполните `terra project init`",
            "Terra project is not initialized in this directory. Run `terra project init` first",
        ),
        DiagKind::ProjectExists => pair(
            "E047",
            ru,
            "в этом каталоге уже есть terra.toml",
            "this directory already contains terra.toml",
        ),
        DiagKind::NotRegistered { name } => (
            "E048",
            if ru {
                format!("проект '{name}' не найден в реестре")
            } else {
                format!("project '{name}' is not in the registry")
            },
        ),
        DiagKind::AmbiguousProject { name } => (
            "E049",
            if ru {
                format!("несколько проектов с именем '{name}'. Укажите каталог")
            } else {
                format!("multiple projects named '{name}'. Use a directory")
            },
        ),
        DiagKind::BadManifest { detail } => ("E050", detail.clone()),
        DiagKind::Git { detail } => ("E051", detail.clone()),
        DiagKind::Build { detail } => ("E052", detail.clone()),
        DiagKind::CompilerMissing { version } => (
            "E053",
            if ru {
                format!("компилятор версии '{version}' не установлен")
            } else {
                format!("compiler version '{version}' is not installed")
            },
        ),
        DiagKind::TargetMissing { target } => (
            "E054",
            if ru {
                format!("цель сборки '{target}' недоступна")
            } else {
                format!("build target '{target}' is not available")
            },
        ),
        DiagKind::BadPackage { name } => (
            "E055",
            if ru {
                format!("некорректное имя пакета '{name}'")
            } else {
                format!("invalid package name '{name}'")
            },
        ),
        DiagKind::BadLang => pair(
            "E056",
            ru,
            "язык должен быть ru или en",
            "language must be ru or en",
        ),
        DiagKind::TopLevelBinding => pair(
            "E057",
            ru,
            "на верхнем уровне допустим только const",
            "only const bindings are allowed at the top level",
        ),
        DiagKind::Unreachable => pair("E058", ru, "недостижимый код", "unreachable code"),
        DiagKind::EmptyArray => pair(
            "E059",
            ru,
            "пустой массив требует аннотацию типа",
            "empty array needs a type annotation",
        ),
        DiagKind::BadSuper => pair(
            "E060",
            ru,
            "super недопустим в этом контексте",
            "super is not valid in this context",
        ),
        DiagKind::StaticSelf => pair(
            "E061",
            ru,
            "self недоступен в статическом контексте",
            "self is not available in a static context",
        ),
        DiagKind::NestedType => pair(
            "E062",
            ru,
            "вложенные типы не поддерживаются",
            "nested types are not supported",
        ),
        DiagKind::MissingHandler => pair(
            "E063",
            ru,
            "try требует обработчик exception",
            "try requires an exception handler",
        ),
        DiagKind::NonExhaustive => pair(
            "E064",
            ru,
            "неполный разбор: добавьте ветку или '_'",
            "non-exhaustive match: add a branch or '_'",
        ),
        DiagKind::BadPattern => pair(
            "E065",
            ru,
            "недопустимый шаблон match",
            "invalid match pattern",
        ),
        DiagKind::ReturnOutside => pair(
            "E066",
            ru,
            "return вне функции",
            "return outside of a function",
        ),
        DiagKind::ConstType { detail } => ("E067", detail.clone()),
        DiagKind::Shadow { name } => (
            "E068",
            if ru {
                format!("имя '{name}' уже объявлено в этой области")
            } else {
                format!("name '{name}' is already declared in this scope")
            },
        ),
        DiagKind::UseAfterScope => pair("E069", ru, "имя вне области видимости", "name is out of scope"),
        DiagKind::AsyncCtor => pair(
            "E070",
            ru,
            "конструктор не может быть async",
            "a constructor cannot be async",
        ),
        DiagKind::EntryCtor { name } => (
            "E071",
            if ru {
                format!("класс точки входа '{name}' должен иметь конструктор без аргументов")
            } else {
                format!("entry class '{name}' needs a constructor with no arguments")
            },
        ),
        DiagKind::NoPackage => pair(
            "E072",
            ru,
            "в файле проекта нужен package",
            "a project file needs a package declaration",
        ),
        DiagKind::Internal { detail } => ("E099", detail.clone()),
    }
}

fn pair(code: &'static str, ru: bool, ru_text: &str, en_text: &str) -> (&'static str, String) {
    (code, if ru { ru_text } else { en_text }.to_string())
}

/// Runtime and debugger phrases.
#[derive(Clone, Debug)]
pub enum RtMsg {
    DivZero,
    NullRef,
    UseAfterFree,
    DoubleFree,
    Index { len: i64, index: i64 },
    Overflow,
    Type { detail: String },
    Cast { detail: String },
    UncaughtHeader,
    At,
    Leak { count: usize },
    Breakpoint { file: String, line: u32 },
    Exited { code: i32 },
    VarsHeader,
    StackHeader,
    NoFrames,
    UnknownVar { name: String },
    Help,
    Prompt,
    UnknownCmd { cmd: String },
    Started,
    StepWhere { func: String, file: String, line: u32 },
}

pub fn rt(lang: Lang, msg: RtMsg) -> String {
    let ru = lang == Lang::Ru;
    match msg {
        RtMsg::DivZero => {
            if ru {
                "деление на ноль".into()
            } else {
                "division by zero".into()
            }
        }
        RtMsg::NullRef => {
            if ru {
                "обращение к null".into()
            } else {
                "null reference".into()
            }
        }
        RtMsg::UseAfterFree => {
            if ru {
                "использование освобождённой памяти".into()
            } else {
                "use after free".into()
            }
        }
        RtMsg::DoubleFree => {
            if ru {
                "повторное освобождение памяти".into()
            } else {
                "double free".into()
            }
        }
        RtMsg::Index { len, index } => {
            if ru {
                format!("индекс {index} вне границ длины {len}")
            } else {
                format!("index {index} out of bounds for length {len}")
            }
        }
        RtMsg::Overflow => {
            if ru {
                "переполнение".into()
            } else {
                "numeric overflow".into()
            }
        }
        RtMsg::Type { detail } => {
            if ru {
                format!("ошибка типа во время выполнения: {detail}")
            } else {
                format!("runtime type error: {detail}")
            }
        }
        RtMsg::Cast { detail } => {
            if ru {
                format!("не удалось преобразовать значение: {detail}")
            } else {
                format!("conversion failed: {detail}")
            }
        }
        RtMsg::UncaughtHeader => {
            if ru {
                "необработанное исключение".into()
            } else {
                "uncaught exception".into()
            }
        }
        RtMsg::At => if ru { "в".into() } else { "at".into() },
        RtMsg::Leak { count } => {
            if ru {
                format!("предупреждение: не освобождено объектов в куче: {count}")
            } else {
                format!("warning: {count} heap object(s) leaked")
            }
        }
        RtMsg::Breakpoint { file, line } => {
            if ru {
                format!("точка останова {file}:{line}")
            } else {
                format!("breakpoint at {file}:{line}")
            }
        }
        RtMsg::Exited { code } => {
            if ru {
                format!("процесс завершился с кодом {code}")
            } else {
                format!("process exited with code {code}")
            }
        }
        RtMsg::VarsHeader => if ru { "переменные:".into() } else { "variables:".into() },
        RtMsg::StackHeader => if ru { "стек вызовов:".into() } else { "call stack:".into() },
        RtMsg::NoFrames => {
            if ru {
                "нет активного кадра".into()
            } else {
                "no active frame".into()
            }
        }
        RtMsg::UnknownVar { name } => {
            if ru {
                format!("нет переменной '{name}'")
            } else {
                format!("no variable '{name}'")
            }
        }
        RtMsg::Help => {
            if ru {
                "команды: b <файл>:<строка>, run, next, step, out, vars, p <имя>, bt, c, q".into()
            } else {
                "commands: b <file>:<line>, run, next, step, out, vars, p <name>, bt, c, q".into()
            }
        }
        RtMsg::Prompt => "(terra) ".into(),
        RtMsg::UnknownCmd { cmd } => {
            if ru {
                format!("неизвестная команда '{cmd}'. Введите help")
            } else {
                format!("unknown command '{cmd}'. Type help")
            }
        }
        RtMsg::Started => {
            if ru {
                "отладчик Terra. Введите help".into()
            } else {
                "Terra debugger. Type help".into()
            }
        }
        RtMsg::StepWhere { func, file, line } => {
            if ru {
                format!("останов в {func} ({file}:{line})")
            } else {
                format!("stopped in {func} ({file}:{line})")
            }
        }
    }
}
