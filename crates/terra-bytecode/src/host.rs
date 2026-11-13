//! Host functions implemented by the virtual machine and imported with `use`.
//!
//! The id of a function is its index in [`HOSTS`]. Terra has no generics;
//! collection values use the dynamic type `any`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostTy {
    Void,
    Int,
    Float,
    Double,
    Bool,
    String,
    Any,
}

#[derive(Clone, Copy, Debug)]
pub struct HostFn {
    pub package: &'static str,
    pub name: &'static str,
    pub params: &'static [HostTy],
    pub ret: HostTy,
    pub doc_en: &'static str,
    pub doc_ru: &'static str,
}

pub fn host(id: u16) -> Option<&'static HostFn> {
    HOSTS.get(id as usize)
}

pub fn host_api_json() -> std::string::String {
    let mut out = std::string::String::from("[\n");
    for (id, f) in HOSTS.iter().enumerate() {
        if id != 0 {
            out.push_str(",\n");
        }
        let params: Vec<std::string::String> = f.params.iter().map(|p| format!("\"{}\"", host_ty_name(*p))).collect();
        out.push_str(&format!(
            "  {{\"id\":{id},\"package\":\"{}\",\"name\":\"{}\",\"params\":[{}],\"ret\":\"{}\",\"doc_en\":{},\"doc_ru\":{}}}",
            f.package,
            f.name,
            params.join(","),
            host_ty_name(f.ret),
            json_str(f.doc_en),
            json_str(f.doc_ru),
        ));
    }
    out.push_str("\n]\n");
    out
}

pub fn host_ty_name(ty: HostTy) -> &'static str {
    match ty {
        HostTy::Void => "void",
        HostTy::Int => "int",
        HostTy::Float => "float",
        HostTy::Double => "double",
        HostTy::Bool => "bool",
        HostTy::String => "string",
        HostTy::Any => "any",
    }
}

fn json_str(s: &str) -> std::string::String {
    let mut out = std::string::String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

macro_rules! h {
    ($pkg:literal, $name:literal, [$($p:expr),* $(,)?], $ret:expr, $en:literal, $ru:literal) => {
        HostFn {
            package: $pkg,
            name: $name,
            params: &[$($p),*],
            ret: $ret,
            doc_en: $en,
            doc_ru: $ru,
        }
    };
}

use HostTy::*;

pub static HOSTS: &[HostFn] = &[
    h!("com.terra.math", "sin", [Double], Double, "Sine of radians.", "Синус радиан."),
    h!("com.terra.math", "cos", [Double], Double, "Cosine of radians.", "Косинус радиан."),
    h!("com.terra.math", "tan", [Double], Double, "Tangent of radians.", "Тангенс радиан."),
    h!("com.terra.math", "asin", [Double], Double, "Arc sine.", "Арксинус."),
    h!("com.terra.math", "acos", [Double], Double, "Arc cosine.", "Арккосинус."),
    h!("com.terra.math", "atan", [Double], Double, "Arc tangent.", "Арктангенс."),
    h!("com.terra.math", "atan2", [Double, Double], Double, "Arc tangent of y/x.", "Арктангенс y/x."),
    h!("com.terra.math", "sqrt", [Double], Double, "Square root.", "Квадратный корень."),
    h!("com.terra.math", "pow", [Double, Double], Double, "base raised to exp.", "base в степени exp."),
    h!("com.terra.math", "log", [Double], Double, "Natural logarithm.", "Натуральный логарифм."),
    h!("com.terra.math", "log10", [Double], Double, "Base-10 logarithm.", "Десятичный логарифм."),
    h!("com.terra.math", "exp", [Double], Double, "e raised to x.", "Экспонента."),
    h!("com.terra.math", "floor", [Double], Double, "Largest integer not above x.", "Пол вниз."),
    h!("com.terra.math", "ceil", [Double], Double, "Smallest integer not below x.", "Пол вверх."),
    h!("com.terra.math", "round", [Double], Double, "Round to nearest integer.", "Округление."),
    h!("com.terra.math", "abs", [Double], Double, "Absolute value.", "Модуль."),
    h!("com.terra.math", "min", [Double, Double], Double, "Smaller of two numbers.", "Меньшее из двух."),
    h!("com.terra.math", "max", [Double, Double], Double, "Larger of two numbers.", "Большее из двух."),
    h!("com.terra.math", "pi", [], Double, "Pi.", "Число пи."),
    h!("com.terra.math", "e", [], Double, "Euler's number.", "Число e."),
    h!("com.terra.fs", "read_text", [String], String, "Read a UTF-8 file.", "Прочитать файл UTF-8."),
    h!("com.terra.fs", "write_text", [String, String], Void, "Create or replace a text file.", "Создать или заменить текстовый файл."),
    h!("com.terra.fs", "append_text", [String, String], Void, "Append text to a file.", "Дописать текст в файл."),
    h!("com.terra.fs", "exists", [String], Bool, "True when the path exists.", "Истина, если путь существует."),
    h!("com.terra.fs", "is_file", [String], Bool, "True when the path is a file.", "Истина, если путь — файл."),
    h!("com.terra.fs", "is_dir", [String], Bool, "True when the path is a directory.", "Истина, если путь — каталог."),
    h!("com.terra.fs", "remove", [String], Void, "Delete a file or directory tree.", "Удалить файл или дерево каталога."),
    h!("com.terra.fs", "mkdir", [String], Void, "Create a directory and its parents.", "Создать каталог вместе с родителями."),
    h!("com.terra.fs", "list", [String], String, "JSON array of names in a directory.", "JSON-массив имён в каталоге."),
    h!("com.terra.fs", "copy", [String, String], Void, "Copy a file.", "Скопировать файл."),
    h!("com.terra.fs", "rename", [String, String], Void, "Rename or move a path.", "Переименовать или переместить путь."),
    h!("com.terra.fs", "size", [String], Int, "File size in bytes.", "Размер файла в байтах."),
    h!("com.terra.fs", "read_base64", [String], String, "Read a file and return base64.", "Прочитать файл как base64."),
    h!("com.terra.fs", "write_base64", [String, String], Void, "Decode base64 and write a file.", "Декодировать base64 и записать файл."),
    h!("com.terra.env", "get", [String], String, "Environment variable, or an empty string.", "Переменная окружения или пустая строка."),
    h!("com.terra.env", "has", [String], Bool, "True when the variable is set.", "Истина, если переменная задана."),
    h!("com.terra.env", "set", [String, String], Void, "Set an environment variable for this process.", "Задать переменную окружения процесса."),
    h!("com.terra.env", "remove", [String], Void, "Remove an environment variable.", "Удалить переменную окружения."),
    h!("com.terra.env", "all", [], String, "JSON object of the process environment.", "JSON-объект окружения процесса."),
    h!("com.terra.os", "name", [], String, "Operating system name.", "Имя операционной системы."),
    h!("com.terra.os", "arch", [], String, "CPU architecture.", "Архитектура процессора."),
    h!("com.terra.os", "cwd", [], String, "Current working directory.", "Текущий каталог."),
    h!("com.terra.os", "chdir", [String], Void, "Change the working directory.", "Сменить текущий каталог."),
    h!("com.terra.os", "home", [], String, "Home directory.", "Домашний каталог."),
    h!("com.terra.os", "tmp", [], String, "Temporary directory.", "Каталог временных файлов."),
    h!("com.terra.os", "pid", [], Int, "Process id.", "Идентификатор процесса."),
    h!("com.terra.os", "hostname", [], String, "Host name.", "Имя компьютера."),
    h!("com.terra.os", "sleep", [Int], Void, "Sleep for the given milliseconds.", "Подождать указанное число миллисекунд."),
    h!("com.terra.os", "args", [], Any, "Program arguments as a list.", "Аргументы программы как список."),
    h!("com.terra.path", "join", [String, String], String, "Join two path segments.", "Соединить два сегмента пути."),
    h!("com.terra.path", "name", [String], String, "Final component of a path.", "Последний компонент пути."),
    h!("com.terra.path", "parent", [String], String, "Parent directory.", "Родительский каталог."),
    h!("com.terra.path", "ext", [String], String, "Extension without the dot.", "Расширение без точки."),
    h!("com.terra.proc", "output", [String, Any], String, "Run a process. The second argument is a list of string arguments. Returns JSON with code, stdout, and stderr.", "Запустить процесс. Второй аргумент — список строк-аргументов. Возвращает JSON с code, stdout и stderr."),
    h!("com.terra.proc", "exit", [Int], Void, "Exit the process with this status code.", "Завершить процесс с этим кодом."),
    h!("com.terra.http", "get", [String], String, "HTTP GET. Returns the body. Status 400 and above throws IoError.", "HTTP GET. Возвращает тело. Статус 400 и выше бросает IoError."),
    h!("com.terra.http", "text", [String, String, String, String], String, "HTTP request (method, url, content type, body). Returns JSON with status, content_type, and body.", "HTTP-запрос (метод, url, тип, тело). Возвращает JSON со status, content_type и body."),
    h!("com.terra.http", "serve", [Int, String], Void, "Serve HTTP/1.1 on 0.0.0.0 until the process stops. HTTPS is not supported. The handler is `func name(method: string, path: string, body: string) -> string`. A response `CODE type` on the first line sets status and content type; otherwise the text is a 200 body.", "HTTP/1.1 на 0.0.0.0, пока процесс жив. HTTPS нет. Обработчик — `func name(method: string, path: string, body: string) -> string`. Первая строка `CODE type` задаёт статус и тип, иначе текст — тело ответа 200."),
    h!("com.terra.http", "serve_n", [Int, String, Int], Void, "Like http.serve, but returns after N requests.", "Как http.serve, но возвращает управление после N запросов."),
    h!("com.terra.net", "connect", [String, Int], Any, "Open a TCP connection. Returns a socket handle.", "Открыть TCP-соединение. Возвращает сокет."),
    h!("com.terra.net", "send", [Any, String], Void, "Send UTF-8 bytes.", "Отправить байты UTF-8."),
    h!("com.terra.net", "recv", [Any, Int], String, "Read up to N bytes, waiting at most 5 seconds.", "Прочитать до N байт, ожидая не дольше 5 секунд."),
    h!("com.terra.net", "close", [Any], Void, "Close a socket or listener.", "Закрыть сокет или слушатель."),
    h!("com.terra.net", "listen", [Int], Any, "Listen on 0.0.0.0. Port 0 chooses a free port.", "Слушать 0.0.0.0. Порт 0 выбирает свободный порт."),
    h!("com.terra.net", "port", [Any], Int, "Local port of a listener or socket.", "Локальный порт слушателя или сокета."),
    h!("com.terra.net", "accept", [Any], Any, "Accept one TCP connection.", "Принять одно TCP-соединение."),
    h!("com.terra.thread", "spawn", [String, String], Any, "Run `func name(data: string) -> string` on a new OS thread. Returns a handle.", "Запустить `func name(data: string) -> string` в новом потоке ОС. Возвращает дескриптор."),
    h!("com.terra.thread", "join", [Any], String, "Wait for a thread and return its string result.", "Дождаться потока и вернуть строковый результат."),
    h!("com.terra.thread", "sleep", [Int], Void, "Sleep the current thread for milliseconds.", "Усыпить текущий поток на миллисекунды."),
    h!("com.terra.thread", "id", [], Int, "A numeric id of the current OS thread.", "Числовой идентификатор текущего потока ОС."),
    h!("com.terra.json", "parse", [String], Any, "Parse JSON into a value handle.", "Разобрать JSON в значение."),
    h!("com.terra.json", "stringify", [Any], String, "Encode a JSON value, list, or map as JSON text.", "Закодировать JSON, список или словарь в текст."),
    h!("com.terra.json", "kind", [Any], String, "null, bool, number, string, array, or object.", "null, bool, number, string, array или object."),
    h!("com.terra.json", "get", [Any, String], Any, "Object field. Missing fields are null.", "Поле объекта. Отсутствующее поле — null."),
    h!("com.terra.json", "set", [Any, String, Any], Void, "Set an object field.", "Записать поле объекта."),
    h!("com.terra.json", "index", [Any, Int], Any, "Array element.", "Элемент массива."),
    h!("com.terra.json", "len", [Any], Int, "Length of a JSON array, object, list, or map.", "Длина JSON-массива, объекта, списка или словаря."),
    h!("com.terra.json", "push", [Any, Any], Void, "Append to a JSON array.", "Добавить элемент в JSON-массив."),
    h!("com.terra.xml", "parse", [String], Any, "Parse an XML element into JSON: tag, attrs, text, children.", "Разобрать XML-элемент в JSON: tag, attrs, text, children."),
    h!("com.terra.xml", "stringify", [Any], String, "Write that JSON element back to XML.", "Записать такой JSON-элемент обратно в XML."),
    h!("com.terra.yaml", "parse", [String], Any, "Parse YAML into a JSON value.", "Разобрать YAML в JSON-значение."),
    h!("com.terra.yaml", "stringify", [Any], String, "Write a JSON value as YAML.", "Записать JSON-значение как YAML."),
    h!("com.terra.sqlite", "open", [String], Any, "Open or create a SQLite3 database.", "Открыть или создать базу SQLite3."),
    h!("com.terra.sqlite", "exec", [Any, String], Void, "Run SQL that does not return rows.", "Выполнить SQL без выборки строк."),
    h!("com.terra.sqlite", "query", [Any, String], String, "Run a query. Returns a JSON array of objects.", "Выполнить запрос. Возвращает JSON-массив объектов."),
    h!("com.terra.sqlite", "close", [Any], Void, "Close a database handle.", "Закрыть базу."),
    h!("com.terra.archive", "add", [String, String, String], Void, "Add or replace a text entry in a zip file, creating the archive if needed.", "Добавить или заменить текстовую запись в zip, создав архив при необходимости."),
    h!("com.terra.archive", "list", [String], String, "JSON array of entry names.", "JSON-массив имён записей."),
    h!("com.terra.archive", "read", [String, String], String, "Read a zip entry as UTF-8 text.", "Прочитать запись zip как текст UTF-8."),
    h!("com.terra.archive", "extract", [String, String], Void, "Extract a zip archive into a directory.", "Распаковать zip в каталог."),
    h!("com.terra.list", "new", [], Any, "Empty dynamic list.", "Пустой динамический список."),
    h!("com.terra.list", "push", [Any, Any], Void, "Append a value.", "Добавить значение в конец."),
    h!("com.terra.list", "pop", [Any], Any, "Remove and return the last value.", "Снять и вернуть последний элемент."),
    h!("com.terra.list", "get", [Any, Int], Any, "Element at an index.", "Элемент по индексу."),
    h!("com.terra.list", "set", [Any, Int, Any], Void, "Replace an element.", "Заменить элемент."),
    h!("com.terra.list", "len", [Any], Int, "Number of elements.", "Число элементов."),
    h!("com.terra.list", "remove", [Any, Int], Any, "Remove an index and return it.", "Удалить индекс и вернуть значение."),
    h!("com.terra.list", "clear", [Any], Void, "Remove every element.", "Удалить все элементы."),
    h!("com.terra.set", "new", [], Any, "Empty set.", "Пустое множество."),
    h!("com.terra.set", "add", [Any, Any], Void, "Insert a value.", "Добавить значение."),
    h!("com.terra.set", "has", [Any, Any], Bool, "True when the value is present.", "Истина, если значение есть."),
    h!("com.terra.set", "remove", [Any, Any], Void, "Remove a value.", "Удалить значение."),
    h!("com.terra.set", "len", [Any], Int, "Number of values.", "Число значений."),
    h!("com.terra.stack", "new", [], Any, "Empty stack.", "Пустой стек."),
    h!("com.terra.stack", "push", [Any, Any], Void, "Push a value.", "Положить значение."),
    h!("com.terra.stack", "pop", [Any], Any, "Pop a value.", "Снять значение."),
    h!("com.terra.stack", "peek", [Any], Any, "Top value without removing it.", "Верхнее значение без удаления."),
    h!("com.terra.stack", "len", [Any], Int, "Stack depth.", "Глубина стека."),
    h!("com.terra.queue", "new", [], Any, "Empty queue.", "Пустая очередь."),
    h!("com.terra.queue", "push", [Any, Any], Void, "Enqueue a value.", "Поставить значение в очередь."),
    h!("com.terra.queue", "pop", [Any], Any, "Dequeue the oldest value.", "Снять самое старое значение."),
    h!("com.terra.queue", "len", [Any], Int, "Queue length.", "Длина очереди."),
    h!("com.terra.map", "new", [], Any, "Empty string-keyed map.", "Пустой словарь со строковыми ключами."),
    h!("com.terra.map", "set", [Any, String, Any], Void, "Set a key.", "Записать ключ."),
    h!("com.terra.map", "get", [Any, String], Any, "Value for a key, or null.", "Значение по ключу или null."),
    h!("com.terra.map", "has", [Any, String], Bool, "True when the key exists.", "Истина, если ключ есть."),
    h!("com.terra.map", "remove", [Any, String], Void, "Remove a key.", "Удалить ключ."),
    h!("com.terra.map", "len", [Any], Int, "Number of keys.", "Число ключей."),
    h!("com.terra.map", "keys", [Any], Any, "List of keys.", "Список ключей."),
    h!("com.terra.hash", "sha256", [String], String, "SHA-256 hex digest.", "SHA-256 в hex."),
    h!("com.terra.hash", "sha1", [String], String, "SHA-1 hex digest.", "SHA-1 в hex."),
    h!("com.terra.hash", "md5", [String], String, "MD5 hex digest.", "MD5 в hex."),
    h!("com.terra.hash", "fnv", [String], String, "FNV-1a 64-bit hex digest.", "FNV-1a 64-bit в hex."),
    h!("com.terra.time", "now", [], Int, "Unix time in milliseconds.", "Unix-время в миллисекундах."),
    h!("com.terra.time", "format", [Int, String], String, "Format Unix milliseconds. Tokens: YYYY MM DD hh mm ss.", "Формат Unix-миллисекунд. Жетоны: YYYY MM DD hh mm ss."),
    h!("com.terra.time", "parse", [String], Int, "Parse `YYYY-MM-DDTHH:MM:SSZ` or `YYYY-MM-DD HH:MM:SS` as UTC milliseconds.", "Разобрать `YYYY-MM-DDTHH:MM:SSZ` или `YYYY-MM-DD HH:MM:SS` как UTC-миллисекунды."),
    h!("com.terra.time", "parts", [Int], String, "JSON with year, month, day, hour, minute, second in UTC.", "JSON с year, month, day, hour, minute, second в UTC."),
    h!("com.terra.time", "add", [Int, Int], Int, "Add a millisecond delta.", "Прибавить миллисекунды."),
    h!("com.terra.regex", "is_match", [String, String], Bool, "True when pattern matches text. Arguments: pattern, text.", "Истина, если шаблон совпал с текстом. Аргументы: шаблон, текст."),
    h!("com.terra.regex", "find", [String, String], String, "First match of pattern in text, or an empty string.", "Первое совпадение шаблона в тексте или пустая строка."),
    h!("com.terra.regex", "find_all", [String, String], String, "JSON array of matches of pattern in text.", "JSON-массив совпадений шаблона в тексте."),
    h!("com.terra.regex", "replace", [String, String, String], String, "Replace pattern in text. `$1` inserts a capture. Arguments: pattern, text, replacement.", "Заменить шаблон в тексте. `$1` подставляет группу. Аргументы: шаблон, текст, замена."),
    h!("com.terra.regex", "split", [String, String], String, "JSON array of pieces of text split by pattern.", "JSON-массив частей текста, разрезанных по шаблону."),
    h!("com.terra.rand", "int", [Int, Int], Int, "Inclusive random integer.", "Случайное целое, включая границы."),
    h!("com.terra.rand", "float", [], Double, "Random double in [0, 1).", "Случайное double в [0, 1)."),
    h!("com.terra.rand", "seed", [Int], Void, "Seed the process random generator.", "Задать зерно генератора процесса."),
    h!("com.terra.codec", "encode", [String], String, "Base64-encode UTF-8 text.", "Закодировать текст UTF-8 в base64."),
    h!("com.terra.codec", "decode", [String], String, "Decode base64 to UTF-8 text.", "Декодировать base64 в текст UTF-8."),
    h!("com.terra.ui", "page", [String, String, Any, String], Any, "Page: title, theme (roast, instrument, ledger), body, next state JSON.", "Страница: заголовок, тема (roast, instrument, ledger), тело, следующий JSON состояния."),
    h!("com.terra.ui", "col", [Any], Any, "Vertical stack. The argument is one view or an array of views.", "Вертикальный столбец. Аргумент — один вид или массив видов."),
    h!("com.terra.ui", "row", [Any], Any, "Horizontal row that wraps.", "Горизонтальный ряд с переносом."),
    h!("com.terra.ui", "grid", [Int, Any], Any, "Grid with N columns.", "Сетка из N колонок."),
    h!("com.terra.ui", "split", [Any, Any], Any, "Two columns, text then panel.", "Две колонки: текст и панель."),
    h!("com.terra.ui", "sidebar", [Any, Any], Any, "Rail plus main column.", "Боковая колонка и основная."),
    h!("com.terra.ui", "h1", [String], Any, "Page heading.", "Заголовок страницы."),
    h!("com.terra.ui", "h2", [String], Any, "Section heading.", "Заголовок раздела."),
    h!("com.terra.ui", "h3", [String], Any, "Card heading.", "Заголовок карточки."),
    h!("com.terra.ui", "p", [String], Any, "Paragraph.", "Абзац."),
    h!("com.terra.ui", "muted", [String], Any, "Secondary text.", "Вторичный текст."),
    h!("com.terra.ui", "kicker", [String], Any, "Small label above a heading.", "Маленькая подпись над заголовком."),
    h!("com.terra.ui", "display", [String], Any, "Large figure.", "Крупная цифра."),
    h!("com.terra.ui", "nav", [String, Any], Any, "Top bar: brand, then an array of links.", "Верхняя полоса: имя и массив ссылок."),
    h!("com.terra.ui", "a", [String, String], Any, "Link: href, label.", "Ссылка: адрес, подпись."),
    h!("com.terra.ui", "button", [String, String], Any, "Button: label, action name posted back to the view.", "Кнопка: подпись и имя действия, которое вернётся во view."),
    h!("com.terra.ui", "quiet", [String, String], Any, "Quiet button: label, action.", "Тихая кнопка: подпись, действие."),
    h!("com.terra.ui", "submit", [String], Any, "Submit the surrounding form.", "Отправить окружающую форму."),
    h!("com.terra.ui", "card", [String, String, String], Any, "Card: title, body, footer.", "Карточка: заголовок, текст, подвал."),
    h!("com.terra.ui", "hero", [String, String, String, String, String], Any, "Hero: kicker, title, lede, action, button label. An empty action omits the button.", "Герой: подпись, заголовок, текст, действие, подпись кнопки. Пустое действие убирает кнопку."),
    h!("com.terra.ui", "price", [String, String, String], Any, "Menu row: name, note, price.", "Строка меню: имя, заметка, цена."),
    h!("com.terra.ui", "stat", [String, String, String], Any, "Metric: label, value, hint.", "Метрика: подпись, значение, пояснение."),
    h!("com.terra.ui", "badge", [String], Any, "Small status chip.", "Маленькая метка статуса."),
    h!("com.terra.ui", "table", [String, String], Any, "Table. Headers and rows are JSON arrays. Each row is an array of strings.", "Таблица. Заголовки и строки — JSON-массивы. Строка — массив строк."),
    h!("com.terra.ui", "field", [String, String, String], Any, "Text field: label, name, value.", "Поле: подпись, имя, значение."),
    h!("com.terra.ui", "form", [String, Any], Any, "Form: action name, then fields and a submit button.", "Форма: имя действия, затем поля и кнопка отправки."),
    h!("com.terra.ui", "note", [String], Any, "Callout. An empty string renders nothing.", "Заметка. Пустая строка ничего не рисует."),
    h!("com.terra.ui", "device", [Any], Any, "Instrument shell, for a calculator or panel.", "Корпус прибора, для калькулятора или панели."),
    h!("com.terra.ui", "screen", [String], Any, "Readout inside a device.", "Табло внутри прибора."),
    h!("com.terra.ui", "keys", [Any], Any, "Keypad grid.", "Сетка клавиш."),
    h!("com.terra.ui", "key", [String, String, String], Any, "Key: label, action, kind (digit, op, quiet, eq).", "Клавиша: подпись, действие, вид (digit, op, quiet, eq)."),
    h!("com.terra.ui", "anchor", [String, Any], Any, "Section with an id, then the views inside it.", "Секция с id и видами внутри."),
    h!("com.terra.ui", "serve", [Int, String], Void, "Serve the view function on 0.0.0.0. The function is `func name(state: string) -> any` and returns ui.page.", "Отдавать функцию вида на 0.0.0.0. Функция — `func name(state: string) -> any` и возвращает ui.page."),
    h!("com.terra.ui", "serve_n", [Int, String, Int], Void, "Like serve, but returns after N requests.", "Как serve, но возвращает управление после N запросов."),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn host_names_are_unique() {
        let mut seen = HashSet::new();
        for f in HOSTS {
            assert!(seen.insert((f.package, f.name)), "duplicate {}.{}", f.package, f.name);
            assert!(!f.doc_en.is_empty() && !f.doc_ru.is_empty());
        }
        assert!(HOSTS.len() > 100);
    }

    #[test]
    fn docs_api_matches_hosts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        let json = host_api_json();
        let js = format!("window.TERRA_API = {json};\n");
        let json_path = root.join("api.json");
        let js_path = root.join("api.js");
        if !json_path.exists() {
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(&json_path, &json).unwrap();
        }
        if !js_path.exists() {
            std::fs::write(&js_path, &js).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&json_path).unwrap(), json, "docs/api.json drifted from HOSTS");
        assert_eq!(std::fs::read_to_string(&js_path).unwrap(), js, "docs/api.js drifted from HOSTS");
    }
}
