# Terra

Terra — компилируемый язык на Rust. Исходник проходит lexer → parser → AST → семантический анализ → байткод стековой машины → виртуальную машину. `terra build` дописывает этот байткод в копию исполняемого файла (`TERRAOBJ`), и полученный файл запускает программу сам.

## Команды

```bash
terra project init --name MyApp --package com.example.app --compiler latest --version 1.0.0 --lang ru
terra project list
terra run project
terra run project --name MyApp
terra build
terra build --target windows
terra build --target linux
terra debug project
terra git status
```

`terra run project` смотрит только текущий каталог. Без `terra.toml` запуск останавливается с ошибкой. `--name` ищет проект в глобальном реестре (`%APPDATA%\terra\registry.json` или `~/.terra/registry.json`, для тестов — `TERRA_HOME`).

Если флаг не указан: имя — имя каталога, пакет — `com.example.<имя>`, компилятор — `latest` (сейчас 0.1.0), версия — `0.1.0`, язык сообщений — `en`.

Аргументы программы пишутся после `--`:

```bash
terra run project -- one two
```

## Язык

Каждый оператор заканчивается `;`. После `}` у `func`, `class`, `if`, `while`, `for`, `loop`, `match` и `try` точка с запятой не нужна. Лишняя `;` — пустой оператор.

```terra
package com.example.app;

use com.terra.lang;

func main(args: string[]) {
    println("Hello, Terra");
}

func answer() -> int {
    return 42;
}
```

Нет `-> Type` — это не ошибка. Возврат считается `void`, а если все `return` согласованы, тип выводится. Рекурсия без `-> Type` — ошибка.

Точка входа одна: `main` с единственным параметром `string[]`, свободная функция или метод класса. `func main() -> int` синтаксически допустима, но точкой входа не является. Метод экземпляра требует конструктор без параметров.

`const` — константа времени компиляции, `var` — неизменяемая привязка, `mut` — изменяемая. На верхнем уровне только `const`. `new` / `delete` — ручная куча: повторный `delete` и использование старого указателя — исключения. `async` создаёт задачу, `await` выполняет её в том же потоке. `exception (e: DivZero)` ловит исключение после `try`.

## Сборка под платформы

`host` — текущая ОС, копируется запущенный `terra`. `windows` и `linux` используют его же, если хост совпадает. Чужая платформа ищет `TERRA_WINDOWS_LAUNCHER` или `TERRA_LINUX_LAUNCHER`. Если лаунчера нет, пишется `.tbc` и команда сообщает, чего не хватает.

## Стандартная библиотека

Пакет `com.terra.lang` подключается сам: `abs`, `min`, `max` для `int` и класс `Pair`. Остальные пакеты импортируются через `use`.

```terra
use com.terra.fs;

func main(args: string[]) {
    fs_write_text("note.txt", "hi");
    println(fs_read_text("note.txt"));
}
```

Пакеты: `math`, `fs`, `env`, `os`, `path`, `proc`, `http`, `net`, `thread`, `json`, `xml`, `yaml`, `sqlite`, `archive`, `list`, `set`, `stack`, `queue`, `map`, `hash`, `time`, `regex`, `rand`, `codec`. Коллекции имеют тип `any`: обобщений в языке нет. HTTP — только `http://`, сервер слушает `0.0.0.0`. Каждая публичная функция описана в `docs/` на русском и английском. Каталог `docs/api.json` сверяется с таблицей `HOSTS`.

Примеры в `examples/`: `hello`, `cli`, `files`, `json`, `http-get`, `http-server`, `sqlite`, `threads`, `zip`, `regex`. Из каталога примера: `terra run project`. Сайт документации — каталог `docs/`. Workflow `.github/workflows/pages.yml` публикует его на GitHub Pages.

## Структура

```
crates/terra-diagnostics   сообщения en/ru
crates/terra-syntax        lexer, parser, AST
crates/terra-sema          имена, типы, HIR
crates/terra-bytecode      модуль, TBC1, трейлер TERRAOBJ, каталог HOSTS
crates/terra-compiler      драйвер и генерация байткода
crates/terra-runtime       VM, куча, задачи, исключения, hostcall
crates/terra-debugger      точки останова, step, vars, стек
crates/terra-project       terra.toml, реестр, build, git
crates/terra-cli           команда terra
crates/terra-exec          тонкий лаунчер
stdlib/com/terra/lang      Math, Pair
examples/                  hello, cli, files, json, http, sqlite, threads, zip, regex
docs/                      сайт ru/en, api.json
```

Стандартная библиотека подключается к каждой программе. Сторонние исходники лежат в `lib/` и импортируются через `use пакет;` или `use пакет.Имя;`.
