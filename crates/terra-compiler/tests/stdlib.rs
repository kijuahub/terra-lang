use terra_compiler::compile_file;
use terra_diagnostics::Lang;
use terra_runtime::{execute, Outcome};

fn run_args(src: &str, args: &[&str]) -> Outcome {
    let compiled = compile_file("main.terra", src, Lang::En).unwrap_or_else(|e| {
        panic!("{}", e.messages.join("\n"));
    });
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    execute(&compiled.module, &args, Lang::En, false)
}

fn run(src: &str) -> Outcome {
    run_args(src, &[])
}

fn ok(out: Outcome) -> String {
    assert_eq!(out.exit, 0, "stdout:\n{}\nstderr:\n{}", out.stdout, out.stderr);
    assert!(out.stderr.is_empty(), "{}", out.stderr);
    out.stdout.trim().to_string()
}

#[test]
fn math_hash_time_codec_regex_rand() {
    let out = ok(run(
        r#"
        use com.terra.math;
        use com.terra.hash;
        use com.terra.time;
        use com.terra.codec;
        use com.terra.regex;
        use com.terra.rand;
        func main(args: string[]) {
            println(math.sqrt(9.0));
            println(math.pow(2.0, 3.0));
            println(math.floor(3.7));
            println(math.abs(-2.5));
            println(math.min(2.0, 5.0));
            println(math.pi() > 3.0);
            try {
                math.sqrt(-1.0);
                println("nan");
            } exception (e: IoError) {
                println("domain");
            }
            println(hash.sha256("hi"));
            println(hash.sha1("hi"));
            println(hash.md5("hi"));
            println(hash.fnv("hi"));
            println(time.parse("1970-01-01T00:00:00Z"));
            println(time.format(0, "YYYY-MM-DD hh:mm:ss"));
            println(time.add(0, 1500));
            println(time.parts(0));
            println(codec.encode("hi"));
            println(codec.decode("aGk="));
            println(regex.is_match("\\d+", "abc123"));
            println(regex.find("\\d+", "abc123"));
            println(regex.find_all("[a-z]+", "ab 12 cd"));
            println(regex.replace("(\\d)", "a1b2", "[$1]"));
            println(regex.split(",", "a,b,c"));
            rand.seed(7);
            var a = rand.int(0, 1000);
            var f = rand.float();
            rand.seed(7);
            var b = rand.int(0, 1000);
            println(a == b);
            println(a >= 0 && a <= 1000);
            println(f >= 0.0 && f < 1.0);
        }
        "#,
    ));
    let mut lines = out.lines();
    assert_eq!(lines.next().unwrap(), "3.0");
    assert_eq!(lines.next().unwrap(), "8.0");
    assert_eq!(lines.next().unwrap(), "3.0");
    assert_eq!(lines.next().unwrap(), "2.5");
    assert_eq!(lines.next().unwrap(), "2.0");
    assert_eq!(lines.next().unwrap(), "true");
    assert_eq!(lines.next().unwrap(), "domain");
    assert_eq!(lines.next().unwrap(), "8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4");
    assert_eq!(lines.next().unwrap(), "c22b5f9178342609428d6f51b2c5af4c0bde6a42");
    assert_eq!(lines.next().unwrap(), "49f68a5c8493ec2c0bf489821c21fc3b");
    let fnv = lines.next().unwrap();
    assert_eq!(fnv.len(), 16);
    assert!(fnv.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(lines.next().unwrap(), "0");
    assert_eq!(lines.next().unwrap(), "1970-01-01 00:00:00");
    assert_eq!(lines.next().unwrap(), "1500");
    assert!(lines.next().unwrap().contains("\"year\":1970"));
    assert_eq!(lines.next().unwrap(), "aGk=");
    assert_eq!(lines.next().unwrap(), "hi");
    assert_eq!(lines.next().unwrap(), "true");
    assert_eq!(lines.next().unwrap(), "123");
    assert_eq!(lines.next().unwrap(), "[\"ab\",\"cd\"]");
    assert_eq!(lines.next().unwrap(), "a[1]b[2]");
    assert_eq!(lines.next().unwrap(), "[\"a\",\"b\",\"c\"]");
    assert_eq!(lines.next().unwrap(), "true");
    assert_eq!(lines.next().unwrap(), "true");
    assert_eq!(lines.next().unwrap(), "true");
    assert!(lines.next().is_none());
}

#[test]
fn collections() {
    let out = ok(run(
        r#"
        use com.terra.list;
        use com.terra.set;
        use com.terra.stack;
        use com.terra.queue;
        use com.terra.map;
        func main(args: string[]) {
            var xs = list.new();
            list.push(xs, "a");
            list.push(xs, "b");
            println(list.len(xs));
            println(list.get(xs, 0));
            list.set(xs, 1, "c");
            println(list.pop(xs));
            println(list.remove(xs, 0));
            list.push(xs, "z");
            list.clear(xs);
            println(list.len(xs));
            var s = set.new();
            set.add(s, "a");
            set.add(s, "a");
            println(set.len(s));
            println(set.has(s, "a"));
            set.remove(s, "a");
            println(set.has(s, "a"));
            var st = stack.new();
            stack.push(st, "x");
            stack.push(st, "y");
            println(stack.peek(st));
            println(stack.pop(st));
            println(stack.len(st));
            var q = queue.new();
            queue.push(q, "p");
            queue.push(q, "q");
            println(queue.pop(q));
            println(queue.len(q));
            var m = map.new();
            map.set(m, "k", "v");
            println(map.has(m, "k"));
            println(map.get(m, "k"));
            println(map.get(m, "no"));
            println(list.get(map.keys(m), 0));
            map.remove(m, "k");
            println(map.len(m));
        }
        "#,
    ));
    assert_eq!(
        out,
        "2\na\nc\na\n0\n1\ntrue\nfalse\ny\ny\n1\np\n1\ntrue\nv\nnull\nk\n0"
    );
}

#[test]
fn json_xml_yaml() {
    let out = ok(run(
        r#"
        use com.terra.json;
        use com.terra.xml;
        use com.terra.yaml;
        func main(args: string[]) {
            var doc = json.parse("{\"name\":\"Terra\",\"n\":3,\"ok\":true}");
            println(json.kind(doc));
            println(json.get(doc, "name"));
            println(json.get(doc, "n"));
            println(json.get(doc, "ok"));
            println(json.get(doc, "missing"));
            json.set(doc, "name", "Lang");
            println(json.stringify(doc));
            var arr = json.parse("[1,2]");
            json.push(arr, 3);
            println(json.len(arr));
            println(json.index(arr, 2));
            var xml = xml.parse("<note id=\"1\">hi</note>");
            println(json.get(xml, "tag"));
            println(json.get(xml, "text"));
            println(json.get(json.get(xml, "attrs"), "id"));
            println(xml.stringify(xml));
            var y = yaml.parse("name: Terra\nn: 4\n");
            println(json.get(y, "name"));
            println(json.get(y, "n"));
        }
        "#,
    ));
    let lines: Vec<_> = out.lines().collect();
    assert_eq!(lines[0], "object");
    assert_eq!(lines[1], "Terra");
    assert_eq!(lines[2], "3");
    assert_eq!(lines[3], "true");
    assert_eq!(lines[4], "null");
    assert!(lines[5].contains("\"name\":\"Lang\""));
    assert!(lines[5].contains("\"n\":3"));
    assert_eq!(lines[6], "3");
    assert_eq!(lines[7], "3");
    assert_eq!(lines[8], "note");
    assert_eq!(lines[9], "hi");
    assert_eq!(lines[10], "1");
    assert_eq!(lines[11], "<note id=\"1\">hi</note>");
    assert_eq!(lines[12], "Terra");
    assert_eq!(lines[13], "4");
}

#[test]
fn files_env_os_path_and_process() {
    let out = ok(run_args(
        r#"
        use com.terra.fs;
        use com.terra.env;
        use com.terra.os;
        use com.terra.path;
        use com.terra.proc;
        use com.terra.list;
        use com.terra.codec;
        func main(args: string[]) {
            var root = path.join(os.tmp(), "terra-fs-" + to_string(os.pid()));
            fs.mkdir(root);
            var file = path.join(root, "note.txt");
            fs.write_text(file, "hi");
            fs.append_text(file, "!");
            println(fs.read_text(file));
            println(fs.exists(file));
            println(fs.is_file(file));
            println(fs.is_dir(root));
            println(fs.size(file));
            println(path.name(file));
            println(path.ext(file));
            var copy = path.join(root, "copy.txt");
            fs.copy(file, copy);
            var moved = path.join(root, "moved.txt");
            fs.rename(copy, moved);
            println(fs.read_text(moved));
            fs.write_base64(path.join(root, "b.txt"), codec.encode("Z"));
            println(fs.read_text(path.join(root, "b.txt")));
            println(fs.read_base64(file));
            println(fs.list(root));
            fs.remove(root);
            println(fs.exists(root));
            env.set("TERRA_STDLIB_PROBE", "yes");
            println(env.get("TERRA_STDLIB_PROBE"));
            println(env.has("TERRA_STDLIB_PROBE"));
            env.remove("TERRA_STDLIB_PROBE");
            println(env.has("TERRA_STDLIB_PROBE"));
            println(os.name());
            println(os.pid() > 0);
            println(list.len(os.args()));
            println(list.get(os.args(), 0));
            mut cmd: string = "sh";
            mut flag: string = "-c";
            if (os.name() == "windows") {
                cmd = "cmd";
                flag = "/c";
            }
            var pargs = list.new();
            list.push(pargs, flag);
            list.push(pargs, "echo hi");
            println(proc.output(cmd, pargs));
        }
        "#,
        &["alpha"],
    ));
    assert!(out.contains("hi!\ntrue\ntrue\ntrue\n3\nnote.txt\ntxt\nhi!\nZ\naGkh\n"), "{out}");
    assert!(out.contains("[\"b.txt\",\"moved.txt\",\"note.txt\"]"), "{out}");
    assert!(out.contains("false\nyes\ntrue\nfalse\n"), "{out}");
    assert!(out.contains("\ntrue\n1\nalpha\n"), "{out}");
    assert!(out.contains("\"code\":0"), "{out}");
    assert!(out.contains("hi"), "{out}");
}

#[test]
fn sqlite_and_zip() {
    let out = ok(run(
        r#"
        use com.terra.sqlite;
        use com.terra.archive;
        use com.terra.fs;
        use com.terra.os;
        use com.terra.path;
        func main(args: string[]) {
            var dbpath = path.join(os.tmp(), "terra-sql-" + to_string(os.pid()) + ".db");
            if (fs.exists(dbpath)) {
                fs.remove(dbpath);
            }
            var db = sqlite.open(dbpath);
            sqlite.exec(db, "create table notes(body text); insert into notes(body) values ('hello');");
            println(sqlite.query(db, "select body from notes"));
            sqlite.close(db);
            fs.remove(dbpath);
            var z = path.join(os.tmp(), "terra-zip-" + to_string(os.pid()) + ".zip");
            if (fs.exists(z)) {
                fs.remove(z);
            }
            archive.add(z, "a.txt", "alpha");
            archive.add(z, "b.txt", "beta");
            println(archive.list(z));
            println(archive.read(z, "a.txt"));
            var dest = path.join(os.tmp(), "terra-zip-out-" + to_string(os.pid()));
            if (fs.exists(dest)) {
                fs.remove(dest);
            }
            fs.mkdir(dest);
            archive.extract(z, dest);
            println(fs.read_text(path.join(dest, "b.txt")));
            fs.remove(z);
            fs.remove(dest);
        }
        "#,
    ));
    assert_eq!(out, "[{\"body\":\"hello\"}]\n[\"a.txt\",\"b.txt\"]\nalpha\nbeta");
}

#[test]
fn threads_and_tcp() {
    let out = ok(run(
        r#"
        use com.terra.thread;
        use com.terra.net;
        func work(data: string) -> string {
            return "got " + data;
        }
        func client(data: string) -> string {
            var sock = net.connect("127.0.0.1", to_int(data));
            net.send(sock, "ping");
            net.close(sock);
            return "sent";
        }
        func main(args: string[]) {
            var worker = thread.spawn("work", "terra");
            println(thread.join(worker));
            var listener = net.listen(0);
            var port = net.port(listener);
            var peer = thread.spawn("client", to_string(port));
            var sock = net.accept(listener);
            println(net.recv(sock, 16));
            net.close(sock);
            net.close(listener);
            println(thread.join(peer));
        }
        "#,
    ));
    assert_eq!(out, "got terra\nping\nsent");
}

#[test]
fn http_client_and_server() {
    let out = ok(run(
        r#"
        use com.terra.http;
        use com.terra.net;
        use com.terra.thread;
        use com.terra.os;
        func page(method: string, path: string, body: string) -> string {
            return "201 text/plain\n" + path;
        }
        func serve(data: string) -> string {
            http.serve_n(to_int(data), "page", 1);
            return "served";
        }
        func main(args: string[]) {
            try {
                http.get("https://example.com");
                println("https-yes");
            } exception (e: IoError) {
                println("https-no");
            }
            var listener = net.listen(0);
            var port = net.port(listener);
            net.close(listener);
            var worker = thread.spawn("serve", to_string(port));
            mut i: int = 0;
            mut body: string = "";
            while (i < 50) {
                try {
                    body = http.get("http://127.0.0.1:" + to_string(port) + "/ping");
                    i = 50;
                } exception (e: IoError) {
                    os.sleep(50);
                    i = i + 1;
                }
            }
            println(body);
            println(thread.join(worker));
        }
        "#,
    ));
    assert_eq!(out, "https-no\n/ping\nserved");
}

#[test]
fn examples_run() {
    let cases = [
        (include_str!("../../../examples/cli/src/com/example/cli/App.terra"), &["one", "two"][..], "one\ntwo"),
        (include_str!("../../../examples/files/src/com/example/files/App.terra"), &[], "hello files"),
        (include_str!("../../../examples/json/src/com/example/json/App.terra"), &[], "Terra\n2026"),
        (include_str!("../../../examples/http-get/src/com/example/httpget/App.terra"), &[], "usage: terra run project -- http://host/path"),
        (include_str!("../../../examples/http-server/src/com/example/httpserver/App.terra"), &[], "usage: terra run project -- serve PORT"),
        (include_str!("../../../examples/sqlite/src/com/example/sqlite/App.terra"), &[], "[{\"body\":\"hello\"}]"),
        (include_str!("../../../examples/threads/src/com/example/threads/App.terra"), &[], "got terra"),
        (include_str!("../../../examples/zip/src/com/example/zip/App.terra"), &[], "alpha"),
        (include_str!("../../../examples/regex/src/com/example/regex/App.terra"), &[], "123\na[1]b[2]\ntrue"),
        (include_str!("../../../examples/cafe/src/com/example/cafe/App.terra"), &[], "usage: terra run project -- serve 8080"),
        (include_str!("../../../examples/calculator/src/com/example/calculator/App.terra"), &[], "usage: terra run project -- serve 8081"),
        (include_str!("../../../examples/ledger/src/com/example/ledger/App.terra"), &[], "usage: terra run project -- serve 8082"),
    ];
    for (src, args, expect) in cases {
        let out = ok(run_args(src, args));
        assert!(out.contains(expect), "expected {expect} in {out}");
    }
}

#[test]
fn calculator_and_ledger() {
    let calc = ok(run_args(
        include_str!("../../../examples/calculator/src/com/example/calculator/App.terra"),
        &["check"],
    ));
    assert_eq!(calc, "10");
    let book = ok(run_args(
        include_str!("../../../examples/ledger/src/com/example/ledger/App.terra"),
        &["check"],
    ));
    assert!(book.contains("Mina"), "{book}");
    assert!(book.contains("mina@ledger.test"), "{book}");
}

#[test]
fn ui_page_roundtrip() {
    let out = ok(run(
        r#"
        use com.terra.ui;
        use com.terra.http;
        use com.terra.net;
        use com.terra.thread;
        use com.terra.os;
        func view(state: string) -> any {
            return ui.page("North & Bean", "roast", ui.col([
                ui.h1("Coffee, poured slowly."),
                ui.button("Hold", "hold")
            ]), state);
        }
        func serve(data: string) -> string {
            ui.serve_n(to_int(data), "view", 2);
            return "served";
        }
        func main(args: string[]) {
            var listener = net.listen(0);
            var port = net.port(listener);
            net.close(listener);
            var worker = thread.spawn("serve", to_string(port));
            mut i = 0;
            mut body = "";
            while (i < 50) {
                try {
                    body = http.get("http://127.0.0.1:" + to_string(port) + "/");
                    i = 50;
                } exception (e: IoError) {
                    os.sleep(50);
                    i = i + 1;
                }
            }
            println(body);
            println(http.text("POST", "http://127.0.0.1:" + to_string(port) + "/_ui", "application/json", "{\"action\":\"hold\",\"form\":{},\"state\":\"{}\"}"));
            println(thread.join(worker));
        }
        "#,
    ));
    assert!(out.contains("Coffee, poured slowly."), "{out}");
    assert!(out.contains("Inter"), "{out}");
    assert!(out.contains("data-theme=\"roast\""), "{out}");
    assert!(out.contains("hold"), "{out}");
    assert!(out.contains("served"), "{out}");
}
