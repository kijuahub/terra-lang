const KEYS = new Set("class interface extends implements super self override pub priv static final const var mut enum try exception throw if else match loop while for in break continue func return async await true false do package use new delete null".split(" "));

const UI = {
  en: { search: "Search", api: "Standard library", missing: "api.json is missing, so the function list cannot be shown.", empty: "Nothing matches." },
  ru: { search: "Поиск", api: "Стандартная библиотека", missing: "Нет файла api.json, список функций не показан.", empty: "Ничего не найдено." },
};

const SECTIONS = [
  {
    id: "start",
    en: "Start",
    ru: "Начало",
    body: {
      en: `<h1>Terra</h1>
<p>Terra is a compiled language. Source goes through a lexer, a parser, semantic analysis, a stack bytecode machine, and a virtual machine. <code>terra build</code> appends that bytecode to a copy of the executable, and the resulting file runs the program itself.</p>
<pre>package com.example.hello;

use com.terra.fs;

func main(args: string[]) {
    fs_write_text("note.txt", "hi");
    println(fs_read_text("note.txt"));
}</pre>
<p>Build the compiler from the repository, then run a file or a project:</p>
<pre>cargo build -p terra-cli
.\\target\\debug\\terra.exe run .\\examples\\hello\\src\\com\\example\\hello\\App.terra
cd examples\\files
..\\..\\target\\debug\\terra.exe run project</pre>
<p>On Linux and macOS the binary is <code>target/debug/terra</code>. Program arguments go after <code>--</code>.</p>`,
      ru: `<h1>Terra</h1>
<p>Terra — компилируемый язык. Исходник проходит лексический анализ, разбор, семантику, байткод стековой машины и виртуальную машину. <code>terra build</code> дописывает этот байткод в копию исполняемого файла, и полученный файл запускает программу сам.</p>
<pre>package com.example.hello;

use com.terra.fs;

func main(args: string[]) {
    fs_write_text("note.txt", "hi");
    println(fs_read_text("note.txt"));
}</pre>
<p>Соберите компилятор из репозитория и запустите файл или проект:</p>
<pre>cargo build -p terra-cli
.\\target\\debug\\terra.exe run .\\examples\\hello\\src\\com\\example\\hello\\App.terra
cd examples\\files
..\\..\\target\\debug\\terra.exe run project</pre>
<p>В Linux и macOS двоичный файл — <code>target/debug/terra</code>. Аргументы программы пишутся после <code>--</code>.</p>`,
    },
  },
  {
    id: "cli",
    en: "Commands",
    ru: "Команды",
    body: {
      en: `<h2>Commands</h2>
<pre>terra project init --name MyApp --package com.example.app --compiler latest --version 1.0.0 --lang ru
terra project list
terra run project
terra run project --name MyApp -- one two
terra run file.terra
terra build
terra build --target host
terra build --target windows
terra build --target linux
terra build --target all
terra debug project
terra git status
terra git add
terra git commit
terra git log</pre>
<p><code>terra run project</code> reads <code>terra.toml</code> in the current directory only. <code>--name</code> looks the project up in the registry: <code>%APPDATA%\\terra\\registry.json</code> or <code>~/.terra/registry.json</code>. <code>TERRA_HOME</code> overrides that directory.</p>
<p>Defaults: the name is the directory name, the package is <code>com.example.</code> plus the lowercased name, the compiler is <code>latest</code> (this release is 0.1.0), the version is 0.1.0, and messages are English. An unknown compiler version is an error.</p>
<p><code>terra build</code> copies the running <code>terra</code> executable, which recognizes its own trailer, and appends bytecode marked <code>TERRAOBJ</code>. A foreign platform needs <code>TERRA_WINDOWS_LAUNCHER</code> or <code>TERRA_LINUX_LAUNCHER</code>. Without that launcher the command writes a <code>.tbc</code> file and reports what is missing. <code>terra project init</code> runs <code>git init</code> when git is installed.</p>`,
      ru: `<h2>Команды</h2>
<pre>terra project init --name MyApp --package com.example.app --compiler latest --version 1.0.0 --lang ru
terra project list
terra run project
terra run project --name MyApp -- one two
terra run file.terra
terra build
terra build --target host
terra build --target windows
terra build --target linux
terra build --target all
terra debug project
terra git status
terra git add
terra git commit
terra git log</pre>
<p><code>terra run project</code> читает <code>terra.toml</code> только в текущем каталоге. <code>--name</code> ищет проект в реестре: <code>%APPDATA%\\terra\\registry.json</code> или <code>~/.terra/registry.json</code>. Каталог переопределяет <code>TERRA_HOME</code>.</p>
<p>По умолчанию имя — имя каталога, пакет — <code>com.example.</code> и имя в нижнем регистре, компилятор — <code>latest</code> (этот выпуск 0.1.0), версия — 0.1.0, язык сообщений — английский. Неизвестная версия компилятора — ошибка.</p>
<p><code>terra build</code> копирует запущенный <code>terra</code>, который узнаёт свой трейлер, и дописывает байткод с меткой <code>TERRAOBJ</code>. Чужая платформа требует <code>TERRA_WINDOWS_LAUNCHER</code> или <code>TERRA_LINUX_LAUNCHER</code>. Без лаунчера команда пишет <code>.tbc</code> и сообщает, чего не хватает. <code>terra project init</code> выполняет <code>git init</code>, если git установлен.</p>`,
    },
  },
  {
    id: "syntax",
    en: "Syntax",
    ru: "Синтаксис",
    body: {
      en: `<h2>Syntax</h2>
<p>Every statement ends with <code>;</code>. After the closing <code>}</code> of <code>func</code>, <code>class</code>, <code>if</code>, <code>while</code>, <code>for</code>, <code>loop</code>, <code>match</code>, and <code>try</code> a semicolon is not required. An extra <code>;</code> is an empty statement. Braces are required on those block forms. Comments are <code>//</code> to the end of the line and <code>/* ... */</code>.</p>
<p>A missing <code>-&gt; Type</code> is not a syntax error. The return starts as <code>void</code> and is inferred when every <code>return expr</code> agrees. A recursive function without <code>-&gt; Type</code> is an error.</p>
<pre>func answer() -> int {
    return 42;
}

do {
    println("once");
} while (false);</pre>
<p>There is one entry point: <code>main</code> with exactly one parameter of type <code>string[]</code>. The parameter name is free. It may be a free function or a method of a class. <code>func main() -&gt; int</code> is valid syntax and is not an entry. An instance <code>main</code> needs a constructor with no parameters. Two mains are an error. The compiler appends an entry thunk named <code>$entry</code>; it is not part of the source. The debugger does not stop on <code>$entry</code> or on line 0. The entry return must be <code>void</code> or <code>int</code>.</p>`,
      ru: `<h2>Синтаксис</h2>
<p>Каждый оператор заканчивается <code>;</code>. После <code>}</code> у <code>func</code>, <code>class</code>, <code>if</code>, <code>while</code>, <code>for</code>, <code>loop</code>, <code>match</code> и <code>try</code> точка с запятой не нужна. Лишняя <code>;</code> — пустой оператор. У этих форм обязательны фигурные скобки. Комментарии — <code>//</code> до конца строки и <code>/* ... */</code>.</p>
<p>Отсутствие <code>-&gt; Type</code> — не синтаксическая ошибка. Возврат начинается как <code>void</code> и выводится, если все <code>return expr</code> согласованы. Рекурсия без <code>-&gt; Type</code> — ошибка.</p>
<pre>func answer() -> int {
    return 42;
}

do {
    println("once");
} while (false);</pre>
<p>Точка входа одна: <code>main</code> ровно с одним параметром <code>string[]</code>. Имя параметра свободное. Это свободная функция или метод класса. <code>func main() -&gt; int</code> синтаксически допустима и точкой входа не является. Метод экземпляра требует конструктор без параметров. Две функции main — ошибка. Компилятор дописывает переход <code>$entry</code>; в исходнике его нет. Отладчик не останавливается на <code>$entry</code> и на строке 0. Возврат точки входа — <code>void</code> или <code>int</code>.</p>`,
    },
  },
  {
    id: "types",
    en: "Types",
    ru: "Типы",
    body: {
      en: `<h2>Types</h2>
<p>The types are <code>int</code>, <code>float</code>, <code>double</code>, <code>bool</code>, <code>string</code>, arrays such as <code>string[]</code> and <code>int[]</code>, classes, interfaces, enums, and <code>null</code>. Host collections use the dynamic type <code>any</code>. There are no generic types.</p>
<p><code>const</code> is a compile-time immutable value. <code>var</code> is an immutable binding. <code>mut</code> is a mutable binding. Only <code>const</code> is allowed at the top level. Fields need an explicit type. A constructor may assign immutable fields.</p>
<pre>const LIMIT: int = 10;

func main(args: string[]) {
    var name: string = "Terra";
    mut n: int = 0;
    n = n + 1;
    var nums = [1, 2, 3];
    var blank = new int[4];
    delete nums;
    delete blank;
}</pre>
<p>Numeric operations require the same type. A string <code>+</code> concatenates. Widening is only <code>to_int</code>, <code>to_float</code>, and <code>to_double</code>. <code>to_string</code> accepts a value. <code>len</code> accepts an array or a string. <code>println</code> and <code>print</code> accept any type. Integer overflow throws. A floating overflow becomes infinity.</p>`,
      ru: `<h2>Типы</h2>
<p>Типы: <code>int</code>, <code>float</code>, <code>double</code>, <code>bool</code>, <code>string</code>, массивы вроде <code>string[]</code> и <code>int[]</code>, классы, интерфейсы, перечисления и <code>null</code>. Коллекции стандартной библиотеки имеют динамический тип <code>any</code>. Обобщений нет.</p>
<p><code>const</code> — неизменяемое значение времени компиляции. <code>var</code> — неизменяемая привязка. <code>mut</code> — изменяемая привязка. На верхнем уровне допустим только <code>const</code>. У полей тип обязателен. Конструктор может присваивать неизменяемые поля.</p>
<pre>const LIMIT: int = 10;

func main(args: string[]) {
    var name: string = "Terra";
    mut n: int = 0;
    n = n + 1;
    var nums = [1, 2, 3];
    var blank = new int[4];
    delete nums;
    delete blank;
}</pre>
<p>Числовые операции требуют один и тот же тип. <code>+</code> для строк склеивает их. Расширение типа — только <code>to_int</code>, <code>to_float</code> и <code>to_double</code>. <code>to_string</code> принимает значение. <code>len</code> принимает массив или строку. <code>println</code> и <code>print</code> принимают любой тип. Переполнение целого бросает исключение. Переполнение float становится бесконечностью.</p>`,
    },
  },
  {
    id: "operators",
    en: "Operators",
    ru: "Операторы",
    body: {
      en: `<h2>Operators</h2>
<p>Arithmetic: <code>+ - * / %</code>. Comparison: <code>== != &lt; &lt;= &gt; &gt;=</code>. Logic: <code>&amp;&amp; || !</code>. Assignment is <code>=</code>. A range is <code>0..10</code>. Indexing is <code>args[0]</code>. Member access is <code>value.field</code> and <code>value.method()</code>. A qualified call is <code>com.terra.fs.fs_read_text("a")</code>.</p>
<p><code>if</code>, <code>while</code>, and <code>do</code>/<code>while</code> take a boolean. <code>for</code> walks an array or a range, and the C form <code>for (mut i: int = 0; i &lt; n; i = i + 1)</code> is also accepted. <code>loop</code> repeats until <code>break</code>. <code>match</code> compares compile-time constants and <code>_</code>. An enum match is exhaustive or ends with <code>_</code>.</p>
<pre>func main(args: string[]) {
    mut sum: int = 0;
    for (n in [1, 2, 3]) {
        sum = sum + n;
    }
    for (i in 0..3) {
        sum = sum + i;
    }
    match (sum) {
        9 => { println("nine"); }
        _ => { println("other"); }
    }
}</pre>`,
      ru: `<h2>Операторы</h2>
<p>Арифметика: <code>+ - * / %</code>. Сравнение: <code>== != &lt; &lt;= &gt; &gt;=</code>. Логика: <code>&amp;&amp; || !</code>. Присваивание — <code>=</code>. Диапазон — <code>0..10</code>. Индекс — <code>args[0]</code>. Поле и метод — <code>value.field</code> и <code>value.method()</code>. Квалифицированный вызов — <code>com.terra.fs.fs_read_text("a")</code>.</p>
<p><code>if</code>, <code>while</code> и <code>do</code>/<code>while</code> ждут логическое условие. <code>for</code> обходит массив или диапазон; форма <code>for (mut i: int = 0; i &lt; n; i = i + 1)</code> тоже принимается. <code>loop</code> повторяется до <code>break</code>. <code>match</code> сравнивает константы времени компиляции и <code>_</code>. Сопоставление с перечислением исчерпывающее или заканчивается <code>_</code>.</p>
<pre>func main(args: string[]) {
    mut sum: int = 0;
    for (n in [1, 2, 3]) {
        sum = sum + n;
    }
    for (i in 0..3) {
        sum = sum + i;
    }
    match (sum) {
        9 => { println("nine"); }
        _ => { println("other"); }
    }
}</pre>`,
    },
  },
  {
    id: "functions",
    en: "Functions",
    ru: "Функции",
    body: {
      en: `<h2>Functions</h2>
<p>A function is <code>func name(args) -&gt; Type { }</code>. The return type may be omitted. There is no overloading: one name, one function. Functions are not values, so they cannot be stored or passed except by the string name that <code>thread_spawn</code> and <code>http_serve</code> look up.</p>
<pre>func clamp(n: int, lo: int, hi: int) -> int {
    if (n &lt; lo) {
        return lo;
    }
    if (n &gt; hi) {
        return hi;
    }
    return n;
}</pre>
<p>Names of free functions in the module are their simple names. A method name seen by the host is <code>Class.method</code>.</p>`,
      ru: `<h2>Функции</h2>
<p>Функция — <code>func name(args) -&gt; Type { }</code>. Тип возврата можно не писать. Перегрузки нет: одно имя, одна функция. Функции не являются значениями: их нельзя сохранить или передать, кроме строкового имени, которое ищут <code>thread_spawn</code> и <code>http_serve</code>.</p>
<pre>func clamp(n: int, lo: int, hi: int) -> int {
    if (n &lt; lo) {
        return lo;
    }
    if (n &gt; hi) {
        return hi;
    }
    return n;
}</pre>
<p>Имена свободных функций в модуле — простые имена. Имя метода для хоста — <code>Class.method</code>.</p>`,
    },
  },
  {
    id: "classes",
    en: "Classes",
    ru: "Классы",
    body: {
      en: `<h2>Classes</h2>
<p>A class extends one class. The root is <code>Object</code>. A constructor is <code>func ClassName(...)</code>. <code>super()</code> is the first statement of a constructor. If you write no constructor, a public one with no parameters is synthesized. Fields are initialized in order: the superclass, then the field initializers, then the constructor body.</p>
<p><code>pub</code> is visible everywhere. <code>priv</code> is private to the class for members and private to the file for top-level names. The default is package-private. The runtime can still call the entry <code>main</code>. A standard-library constructor that other packages call is <code>pub</code>.</p>
<pre>class Counter extends Object {
    pub mut value: int = 0;
    pub func Counter(start: int) {
        super();
        value = start;
    }
    pub func add(n: int) -> int {
        value = value + n;
        return value;
    }
}</pre>
<p>Reading a superclass field as a value is limited. <code>super.method()</code> calls that method directly. Interface methods are public. The parser does not keep an <code>async</code> flag on an interface method.</p>`,
      ru: `<h2>Классы</h2>
<p>Класс расширяет один класс. Корень — <code>Object</code>. Конструктор — <code>func ClassName(...)</code>. <code>super()</code> — первый оператор конструктора. Если конструктора нет, синтезируется публичный без параметров. Поля инициализируются так: суперкласс, затем инициализаторы полей, затем тело конструктора.</p>
<p><code>pub</code> видно везде. <code>priv</code> для члена — только класс, для верхнего уровня — только файл. По умолчанию имя видно внутри пакета. Точку входа <code>main</code> среда вызывает в любом случае. Конструктор стандартной библиотеки, который вызывают другие пакеты, объявлен <code>pub</code>.</p>
<pre>class Counter extends Object {
    pub mut value: int = 0;
    pub func Counter(start: int) {
        super();
        value = start;
    }
    pub func add(n: int) -> int {
        value = value + n;
        return value;
    }
}</pre>
<p>Чтение поля суперкласса как значения ограничено. <code>super.method()</code> вызывает этот метод напрямую. Методы интерфейса публичные. Разбор не сохраняет признак <code>async</code> у метода интерфейса.</p>`,
    },
  },
  {
    id: "interfaces",
    en: "Interfaces",
    ru: "Интерфейсы",
    body: {
      en: `<h2>Interfaces</h2>
<p>An interface lists methods. A class implements it with <code>implements</code>. The methods are public. A class can implement more than one interface.</p>
<pre>interface Summable {
    func sum() -> int;
}

class Pair extends Object implements Summable {
    pub var first: int;
    pub var second: int;
    pub func Pair(a: int, b: int) {
        super();
        first = a;
        second = b;
    }
    pub func sum() -> int {
        return first + second;
    }
}</pre>
<p><code>com.terra.lang</code> is imported for every program. It contains <code>abs</code>, <code>min</code>, and <code>max</code> for <code>int</code>, and the class <code>Pair</code>.</p>`,
      ru: `<h2>Интерфейсы</h2>
<p>Интерфейс перечисляет методы. Класс реализует его через <code>implements</code>. Методы публичные. Класс может реализовать несколько интерфейсов.</p>
<pre>interface Summable {
    func sum() -> int;
}

class Pair extends Object implements Summable {
    pub var first: int;
    pub var second: int;
    pub func Pair(a: int, b: int) {
        super();
        first = a;
        second = b;
    }
    pub func sum() -> int {
        return first + second;
    }
}</pre>
<p><code>com.terra.lang</code> подключается к каждой программе. Там <code>abs</code>, <code>min</code> и <code>max</code> для <code>int</code> и класс <code>Pair</code>.</p>`,
    },
  },
  {
    id: "enums",
    en: "Enums",
    ru: "Перечисления",
    body: {
      en: `<h2>Enums</h2>
<p>An enum variant carries no payload. Match every variant or end with <code>_</code>.</p>
<pre>enum Color { Red, Green, Blue }

func name(c: Color) -> string {
    match (c) {
        Color.Red => { return "red"; }
        Color.Green => { return "green"; }
        Color.Blue => { return "blue"; }
    }
}</pre>`,
      ru: `<h2>Перечисления</h2>
<p>Вариант перечисления не несёт данных. Нужно покрыть все варианты или закончить веткой <code>_</code>.</p>
<pre>enum Color { Red, Green, Blue }

func name(c: Color) -> string {
    match (c) {
        Color.Red => { return "red"; }
        Color.Green => { return "green"; }
        Color.Blue => { return "blue"; }
    }
}</pre>`,
    },
  },
  {
    id: "memory",
    en: "Memory",
    ru: "Память",
    body: {
      en: `<h2>Memory</h2>
<p><code>new</code> allocates and <code>delete</code> frees. Handles carry a generation, so use after free and a second <code>delete</code> throw. Numbers, booleans, and strings are values. Freeing an array does not free the objects stored in it. A task cannot be deleted. Objects you allocated and did not free are reported on standard error when the program finishes. Host handles, such as a list or a socket, are not counted in that report.</p>
<p>User exceptions are <code>exception Boom;</code>. They extend <code>Exception</code>, whose field 0 is <code>message</code>. Exception declarations have no methods. The keyword is <code>exception (e: Type)</code> after <code>try</code>, not <code>catch</code>.</p>
<pre>func main(args: string[]) {
    try {
        var n = 1 / 0;
        println(n);
    } exception (e: DivZero) {
        println(e.message);
    }
}</pre>
<p>The prelude classes are <code>Object</code>, <code>Exception</code>, <code>DivZero</code>, <code>NullRef</code>, <code>UseAfterFree</code>, <code>IndexError</code>, <code>Overflow</code>, <code>TypeError</code>, <code>CastError</code>, and <code>IoError</code>. The virtual machine finds them by name.</p>`,
      ru: `<h2>Память</h2>
<p><code>new</code> выделяет память, <code>delete</code> освобождает. Указатель хранит поколение, поэтому использование после освобождения и повторный <code>delete</code> бросают исключение. Числа, логические значения и строки — значения. Освобождение массива не освобождает объекты внутри. Задачу удалить нельзя. Объекты, которые вы выделили и не освободили, сообщаются в стандартный поток ошибок в конце программы. Дескрипторы хоста, например список или сокет, в этот отчёт не входят.</p>
<p>Своё исключение — <code>exception Boom;</code>. Оно расширяет <code>Exception</code>, поле 0 которого — <code>message</code>. У объявления исключения нет методов. Ключевое слово после <code>try</code> — <code>exception (e: Type)</code>, не <code>catch</code>.</p>
<pre>func main(args: string[]) {
    try {
        var n = 1 / 0;
        println(n);
    } exception (e: DivZero) {
        println(e.message);
    }
}</pre>
<p>Классы прелюдии: <code>Object</code>, <code>Exception</code>, <code>DivZero</code>, <code>NullRef</code>, <code>UseAfterFree</code>, <code>IndexError</code>, <code>Overflow</code>, <code>TypeError</code>, <code>CastError</code> и <code>IoError</code>. Виртуальная машина ищет их по имени.</p>`,
    },
  },
  {
    id: "async",
    en: "Async",
    ru: "Async",
    body: {
      en: `<h2>Async</h2>
<p>Calling an <code>async</code> function builds a task. The body does not run until <code>await</code>. The scheduler is cooperative and single-threaded. A nested <code>await</code> uses the same call stack. This is not an operating-system thread; see Threads for that.</p>
<pre>async func value() -> int {
    return 7;
}

func main(args: string[]) {
    println(await value());
}</pre>`,
      ru: `<h2>Async</h2>
<p>Вызов <code>async</code>-функции создаёт задачу. Тело не выполняется до <code>await</code>. Планировщик кооперативный и однопоточный. Вложенный <code>await</code> использует тот же стек вызовов. Это не поток операционной системы; поток ОС описан в разделе «Потоки».</p>
<pre>async func value() -> int {
    return 7;
}

func main(args: string[]) {
    println(await value());
}</pre>`,
    },
  },
  {
    id: "threads",
    en: "Threads",
    ru: "Потоки",
    body: {
      en: `<h2>Threads</h2>
<p><code>thread_spawn</code> runs <code>func name(data: string) -&gt; string</code> on a new operating-system thread and returns a handle. <code>thread_join</code> waits and returns that string. Each worker gets a fresh virtual machine, so a host call does not re-enter the paused frame. The payload is a string. There is no shared heap.</p>
<pre>use com.terra.thread;

func work(data: string) -> string {
    return "got " + data;
}

func main(args: string[]) {
    var worker = thread_spawn("work", "terra");
    println(thread_join(worker));
}</pre>
<p><code>thread_sleep</code> pauses the current thread for a number of milliseconds. <code>thread_id</code> returns a number derived from the operating-system thread id. <code>os_sleep</code> does the same pause through the <code>com.terra.os</code> package.</p>`,
      ru: `<h2>Потоки</h2>
<p><code>thread_spawn</code> запускает <code>func name(data: string) -&gt; string</code> в новом потоке операционной системы и возвращает дескриптор. <code>thread_join</code> ждёт и возвращает эту строку. У работника своя виртуальная машина, поэтому вызов хоста не входит повторно в приостановленный кадр. Полезная нагрузка — строка. Общей кучи нет.</p>
<pre>use com.terra.thread;

func work(data: string) -> string {
    return "got " + data;
}

func main(args: string[]) {
    var worker = thread_spawn("work", "terra");
    println(thread_join(worker));
}</pre>
<p><code>thread_sleep</code> останавливает текущий поток на миллисекунды. <code>thread_id</code> возвращает число, полученное из идентификатора потока ОС. <code>os_sleep</code> делает ту же паузу через пакет <code>com.terra.os</code>.</p>`,
    },
  },
  {
    id: "debugger",
    en: "Debugger",
    ru: "Отладчик",
    body: {
      en: `<h2>Debugger</h2>
<p><code>terra debug project</code> or <code>terra debug file.terra</code> starts the debugger. Messages follow the project language.</p>
<ul>
<li><code>b file:line</code>, also <code>break</code> and <code>breakpoint</code>, sets a breakpoint.</li>
<li><code>run</code> or <code>c</code> continues.</li>
<li><code>next</code> or <code>n</code> steps over.</li>
<li><code>step</code> or <code>s</code> steps into.</li>
<li><code>out</code> or <code>o</code> steps out.</li>
<li><code>vars</code> or <code>v</code> prints locals.</li>
<li><code>p name</code> prints one value.</li>
<li><code>bt</code> prints the stack.</li>
<li><code>help</code> prints the command list.</li>
<li><code>q</code> quits.</li>
</ul>
<p>Stops on <code>$entry</code> and on line 0 are skipped.</p>`,
      ru: `<h2>Отладчик</h2>
<p><code>terra debug project</code> или <code>terra debug file.terra</code> запускает отладчик. Сообщения идут на языке проекта.</p>
<ul>
<li><code>b file:line</code>, а также <code>break</code> и <code>breakpoint</code>, ставит точку останова.</li>
<li><code>run</code> или <code>c</code> продолжает выполнение.</li>
<li><code>next</code> или <code>n</code> шагает через вызов.</li>
<li><code>step</code> или <code>s</code> входит в вызов.</li>
<li><code>out</code> или <code>o</code> выходит из функции.</li>
<li><code>vars</code> или <code>v</code> печатает локальные переменные.</li>
<li><code>p name</code> печатает одно значение.</li>
<li><code>bt</code> печатает стек.</li>
<li><code>help</code> печатает список команд.</li>
<li><code>q</code> выходит.</li>
</ul>
<p>Остановки на <code>$entry</code> и на строке 0 пропускаются.</p>`,
    },
  },
  {
    id: "packages",
    en: "Packages",
    ru: "Пакеты",
    body: {
      en: `<h2>Packages and projects</h2>
<p>A project file declares its package, and the directory under <code>src/</code> must match. <code>src/com/example/app/App.terra</code> declares <code>package com.example.app;</code>. A single file may omit the package; it then lives in <code>_main</code>.</p>
<pre>package com.example.app;

use com.terra.fs;
use com.terra.json;</pre>
<p><code>use pkg;</code> imports every public name. The first import wins when two packages export the same name, which is why host functions are prefixed (<code>list_push</code>, not <code>push</code>). <code>use pkg.Type;</code> imports one name. Sources in <code>lib/</code> are compiled with the project.</p>
<p>Working examples live in <code>examples/</code>: <code>hello</code>, <code>cli</code>, <code>files</code>, <code>json</code>, <code>http-get</code>, <code>http-server</code>, <code>sqlite</code>, <code>threads</code>, <code>zip</code>, and <code>regex</code>. From a project directory, <code>terra run project</code> runs it. <code>http-get</code> and <code>http-server</code> print a usage line until you pass arguments after <code>--</code>.</p>`,
      ru: `<h2>Пакеты и проекты</h2>
<p>Файл проекта объявляет пакет, и каталог под <code>src/</code> должен ему соответствовать. <code>src/com/example/app/App.terra</code> содержит <code>package com.example.app;</code>. У одиночного файла пакет можно не писать; тогда он попадает в <code>_main</code>.</p>
<pre>package com.example.app;

use com.terra.fs;
use com.terra.json;</pre>
<p><code>use pkg;</code> импортирует все публичные имена. При совпадении имён побеждает первый импорт, поэтому функции хоста имеют приставку (<code>list_push</code>, а не <code>push</code>). <code>use pkg.Type;</code> импортирует одно имя. Исходники в <code>lib/</code> компилируются вместе с проектом.</p>
<p>Рабочие примеры лежат в <code>examples/</code>: <code>hello</code>, <code>cli</code>, <code>files</code>, <code>json</code>, <code>http-get</code>, <code>http-server</code>, <code>sqlite</code>, <code>threads</code>, <code>zip</code> и <code>regex</code>. Из каталога проекта их запускает <code>terra run project</code>. <code>http-get</code> и <code>http-server</code> печатают подсказку, пока аргументы не переданы после <code>--</code>.</p>`,
    },
  },
  {
    id: "stdlib",
    en: "Library",
    ru: "Библиотека",
    body: {
      en: `<h2>Standard library</h2>
<p>Import a package and call its functions. Collections are dynamic <code>any</code> values because Terra has no generics. JSON, XML, and YAML share one JSON value. XML has the fields <code>tag</code>, <code>attrs</code>, <code>text</code>, and <code>children</code>. <code>json_get</code> of an object or array returns a clone: changing the child does not change the parent. Map keys are strings. A missing map or JSON field is <code>null</code>.</p>
<p>HTTP is HTTP/1.1 on <code>http://</code> only. <code>https://</code> throws <code>IoError</code>. The client follows up to five GET redirects. The server listens on <code>0.0.0.0</code>. A handler is <code>func name(method: string, path: string, body: string) -&gt; string</code>. If the first line is <code>CODE content-type</code>, that sets the status and type. Otherwise the whole string is a 200 text body. <code>http_serve_n</code> returns after N requests. Each request runs in a fresh virtual machine.</p>
<p>SQLite is real SQLite 3. <code>sqlite_query</code> returns a JSON array of objects. Zip entry names that contain <code>..</code> or <code>\\</code> are rejected. Dates and times are UTC. Format tokens are <code>YYYY MM DD hh mm ss</code>. <code>time_parse</code> accepts <code>YYYY-MM-DDTHH:MM:SSZ</code> or <code>YYYY-MM-DD HH:MM:SS</code>. <code>rand_seed</code> seeds one generator for the process. <code>proc_exit</code> ends the process. <code>os_chdir</code> changes the process directory.</p>
<p>Regex arguments are the pattern, then the text. <code>regex_replace</code> takes the pattern, the text, and the replacement. <code>$1</code> inserts a capture group, so the pattern needs parentheses. <code>$0</code> is the whole match.</p>
<p>Every public host function is listed below. The text is the same text the compiler ships.</p>
<div id="api"></div>`,
      ru: `<h2>Стандартная библиотека</h2>
<p>Импортируйте пакет и вызывайте его функции. Коллекции имеют динамический тип <code>any</code>, потому что обобщений в Terra нет. JSON, XML и YAML используют одно JSON-значение. У XML поля <code>tag</code>, <code>attrs</code>, <code>text</code> и <code>children</code>. <code>json_get</code> у объекта или массива возвращает копию: изменение потомка не меняет родителя. Ключи словаря — строки. Отсутствующее поле словаря или JSON — <code>null</code>.</p>
<p>HTTP — это HTTP/1.1 и только схема <code>http://</code>. <code>https://</code> бросает <code>IoError</code>. Клиент идёт по редиректам GET не больше пяти раз. Сервер слушает <code>0.0.0.0</code>. Обработчик — <code>func name(method: string, path: string, body: string) -&gt; string</code>. Если первая строка — <code>CODE content-type</code>, она задаёт статус и тип. Иначе вся строка — тело ответа 200. <code>http_serve_n</code> возвращает управление после N запросов. Каждый запрос выполняется в новой виртуальной машине.</p>
<p>SQLite — это настоящий SQLite 3. <code>sqlite_query</code> возвращает JSON-массив объектов. Имена записей zip с <code>..</code> или <code>\\</code> отвергаются. Дата и время — UTC. Жетоны формата: <code>YYYY MM DD hh mm ss</code>. <code>time_parse</code> принимает <code>YYYY-MM-DDTHH:MM:SSZ</code> или <code>YYYY-MM-DD HH:MM:SS</code>. <code>rand_seed</code> задаёт один генератор на процесс. <code>proc_exit</code> завершает процесс. <code>os_chdir</code> меняет каталог процесса.</p>
<p>Аргументы регулярного выражения — сначала шаблон, потом текст. <code>regex_replace</code> принимает шаблон, текст и замену. <code>$1</code> подставляет группу захвата, поэтому в шаблоне нужны скобки. <code>$0</code> — всё совпадение.</p>
<p>Ниже перечислена каждая публичная функция хоста. Текст совпадает с тем, который поставляет компилятор.</p>
<div id="api"></div>`,
    },
  },
  {
    id: "faq",
    en: "FAQ",
    ru: "Вопросы",
    body: {
      en: `<h2>FAQ</h2>
<p><strong>Where is the semicolon?</strong> After a statement. Not after the <code>}</code> that closes a function, class, or control block.</p>
<p><strong>Why is my main ignored?</strong> The entry needs exactly one <code>string[]</code> parameter. A return of <code>void</code> or <code>int</code> is required. A class instance method also needs a zero-argument constructor.</p>
<p><strong>Why did use hide a function?</strong> Two imported names collided and the first one won. Call the other one by its full path, or import only that name.</p>
<p><strong>Can I write a generic list?</strong> No. Use <code>com.terra.list</code> and the other collection packages. Values are <code>any</code>, and a wrong runtime value throws.</p>
<p><strong>Can I open an HTTPS URL?</strong> No. The client accepts <code>http://</code> only.</p>
<p><strong>Does await use a thread pool?</strong> No. <code>await</code> is cooperative on one thread. <code>thread_spawn</code> is a separate operating-system thread with a string argument and a string result.</p>
<p><strong>What time zone is time_now?</strong> UTC, as Unix milliseconds. There is no time-zone database.</p>
<p><strong>The program printed a leak.</strong> A <code>new</code> object or array was still alive at exit. Host values are not part of that count. Free the ones you allocated, or accept the report.</p>
<p><strong>How do I publish the docs?</strong> The site is the <code>docs/</code> directory. GitHub Actions workflow <code>pages</code> uploads that directory. Open <code>docs/index.html</code> through a local static server if the browser blocks <code>api.json</code> from a <code>file://</code> page.</p>`,
      ru: `<h2>Вопросы</h2>
<p><strong>Куда ставить точку с запятой?</strong> После оператора. Не после <code>}</code>, которая закрывает функцию, класс или блок управления.</p>
<p><strong>Почему main не запускается?</strong> У точки входа ровно один параметр <code>string[]</code>. Возврат — <code>void</code> или <code>int</code>. Методу экземпляра нужен ещё конструктор без параметров.</p>
<p><strong>Почему use спрятал функцию?</strong> Два импортированных имени совпали, и победило первое. Вызовите второе полным путём или импортируйте только его.</p>
<p><strong>Можно ли написать обобщённый список?</strong> Нет. Используйте <code>com.terra.list</code> и остальные пакеты коллекций. Значения имеют тип <code>any</code>, а неверное значение во время выполнения бросает исключение.</p>
<p><strong>Можно ли открыть HTTPS?</strong> Нет. Клиент принимает только <code>http://</code>.</p>
<p><strong>await берёт пул потоков?</strong> Нет. <code>await</code> кооперативный и идёт в одном потоке. <code>thread_spawn</code> — отдельный поток операционной системы со строковым аргументом и строковым результатом.</p>
<p><strong>В каком поясе time_now?</strong> UTC, миллисекунды Unix. Базы часовых поясов нет.</p>
<p><strong>Программа написала про утечку.</strong> Объект или массив из <code>new</code> остался жив к выходу. Значения хоста в этот счёт не входят. Освободите выделенное или примите отчёт.</p>
<p><strong>Как опубликовать документацию?</strong> Сайт — это каталог <code>docs/</code>. Workflow <code>pages</code> в GitHub Actions выкладывает этот каталог. Если браузер блокирует <code>api.json</code> со страницы <code>file://</code>, откройте <code>docs/index.html</code> через локальный статический сервер.</p>`,
    },
  },
];

let lang = localStorage.getItem("terra-docs-lang");
if (lang !== "en" && lang !== "ru") {
  lang = (navigator.language || "").toLowerCase().startsWith("ru") ? "ru" : "en";
}
let api = null;
let query = "";

function esc(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function highlight(src) {
  let out = "";
  let i = 0;
  while (i < src.length) {
    if (src.startsWith("//", i)) {
      let j = src.indexOf("\n", i);
      if (j < 0) j = src.length;
      out += `<span class="cm">${esc(src.slice(i, j))}</span>`;
      i = j;
      continue;
    }
    if (src[i] === '"') {
      let j = i + 1;
      while (j < src.length && src[j] !== '"') {
        if (src[j] === "\\") j += 1;
        j += 1;
      }
      if (j < src.length) j += 1;
      out += `<span class="str">${esc(src.slice(i, j))}</span>`;
      i = j;
      continue;
    }
    if (/[A-Za-z_]/.test(src[i])) {
      let j = i + 1;
      while (j < src.length && /[A-Za-z0-9_]/.test(src[j])) j += 1;
      const word = src.slice(i, j);
      out += KEYS.has(word) ? `<span class="kw">${word}</span>` : esc(word);
      i = j;
      continue;
    }
    out += esc(src[i]);
    i += 1;
  }
  return out;
}

function signature(fn) {
  const ret = fn.ret === "void" ? "" : " -> " + fn.ret;
  return fn.name + "(" + fn.params.join(", ") + ")" + ret;
}

function renderApi(root) {
  const box = root.querySelector("#api");
  if (!box) return;
  if (!api) {
    box.textContent = UI[lang].missing;
    return;
  }
  const groups = new Map();
  for (const fn of api) {
    const doc = lang === "ru" ? fn.doc_ru : fn.doc_en;
    const blob = (fn.package + " " + fn.name + " " + doc).toLowerCase();
    if (query && !blob.includes(query)) continue;
    if (!groups.has(fn.package)) groups.set(fn.package, []);
    groups.get(fn.package).push(fn);
  }
  if (groups.size === 0) {
    box.textContent = UI[lang].empty;
    return;
  }
  let html = "";
  for (const [pkg, fns] of groups) {
    html += `<section class="api"><h3>${esc(pkg)}</h3>`;
    for (const fn of fns) {
      const doc = lang === "ru" ? fn.doc_ru : fn.doc_en;
      html += `<article class="fn"><div class="sig">${esc(signature(fn))}</div><p>${esc(doc)}</p></article>`;
    }
    html += "</section>";
  }
  box.innerHTML = html;
}

function render() {
  document.documentElement.lang = lang;
  document.getElementById("q").placeholder = UI[lang].search;
  document.getElementById("lang-en").setAttribute("aria-pressed", lang === "en" ? "true" : "false");
  document.getElementById("lang-ru").setAttribute("aria-pressed", lang === "ru" ? "true" : "false");
  const nav = document.getElementById("nav");
  const main = document.getElementById("main");
  nav.innerHTML = "";
  main.innerHTML = "";
  for (const section of SECTIONS) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = section[lang];
    button.addEventListener("click", () => {
      document.getElementById(section.id).scrollIntoView();
    });
    nav.appendChild(button);
    const el = document.createElement("section");
    el.id = section.id;
    el.innerHTML = section.body[lang];
    main.appendChild(el);
    el.querySelectorAll("pre").forEach((pre) => {
      pre.innerHTML = highlight(pre.textContent.replace(/^\n/, "").replace(/\n$/, ""));
    });
  }
  renderApi(main);
  applyFilter();
}

function applyFilter() {
  const q = query.trim().toLowerCase();
  let any = false;
  for (const section of SECTIONS) {
    const el = document.getElementById(section.id);
    const text = (section[lang] + " " + el.textContent).toLowerCase();
    const show = !q || text.includes(q);
    el.classList.toggle("hidden", !show);
    if (show) any = true;
  }
  document.querySelectorAll("#nav button").forEach((button, i) => {
    const el = document.getElementById(SECTIONS[i].id);
    button.classList.toggle("hidden", el.classList.contains("hidden"));
  });
  const old = document.getElementById("none");
  if (old) old.remove();
  if (!any) {
    const note = document.createElement("p");
    note.id = "none";
    note.textContent = UI[lang].empty;
    document.getElementById("main").appendChild(note);
  }
}

document.getElementById("q").addEventListener("input", (event) => {
  query = event.target.value;
  renderApi(document.getElementById("main"));
  applyFilter();
});

document.querySelectorAll(".langs button").forEach((button) => {
  button.addEventListener("click", () => {
    lang = button.dataset.lang;
    localStorage.setItem("terra-docs-lang", lang);
    const q = document.getElementById("q").value;
    render();
    document.getElementById("q").value = q;
  });
});

fetch("api.json")
  .then((res) => (res.ok ? res.json() : Promise.reject(new Error("missing"))))
  .then((data) => {
    api = data;
    renderApi(document.getElementById("main"));
    applyFilter();
  })
  .catch(() => {
    api = null;
    renderApi(document.getElementById("main"));
  });

render();
