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
            println(math_sqrt(9.0));
            println(math_pow(2.0, 3.0));
            println(math_floor(3.7));
            println(math_abs(-2.5));
            println(math_min(2.0, 5.0));
            println(math_pi() > 3.0);
            try {
                math_sqrt(-1.0);
                println("nan");
            } exception (e: IoError) {
                println("domain");
            }
            println(hash_sha256("hi"));
            println(hash_sha1("hi"));
            println(hash_md5("hi"));
            println(hash_fnv("hi"));
            println(time_parse("1970-01-01T00:00:00Z"));
            println(time_format(0, "YYYY-MM-DD hh:mm:ss"));
            println(time_add(0, 1500));
            println(time_parts(0));
            println(b64_encode("hi"));
            println(b64_decode("aGk="));
            println(regex_is_match("\\d+", "abc123"));
            println(regex_find("\\d+", "abc123"));
            println(regex_find_all("[a-z]+", "ab 12 cd"));
            println(regex_replace("(\\d)", "a1b2", "[$1]"));
            println(regex_split(",", "a,b,c"));
            rand_seed(7);
            var a = rand_int(0, 1000);
            var f = rand_float();
            rand_seed(7);
            var b = rand_int(0, 1000);
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
            var xs = list_new();
            list_push(xs, "a");
            list_push(xs, "b");
            println(list_len(xs));
            println(list_get(xs, 0));
            list_set(xs, 1, "c");
            println(list_pop(xs));
            println(list_remove(xs, 0));
            list_push(xs, "z");
            list_clear(xs);
            println(list_len(xs));
            var s = set_new();
            set_add(s, "a");
            set_add(s, "a");
            println(set_len(s));
            println(set_has(s, "a"));
            set_remove(s, "a");
            println(set_has(s, "a"));
            var st = stack_new();
            stack_push(st, "x");
            stack_push(st, "y");
            println(stack_peek(st));
            println(stack_pop(st));
            println(stack_len(st));
            var q = queue_new();
            queue_push(q, "p");
            queue_push(q, "q");
            println(queue_pop(q));
            println(queue_len(q));
            var m = map_new();
            map_set(m, "k", "v");
            println(map_has(m, "k"));
            println(map_get(m, "k"));
            println(map_get(m, "no"));
            println(list_get(map_keys(m), 0));
            map_remove(m, "k");
            println(map_len(m));
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
            var doc = json_parse("{\"name\":\"Terra\",\"n\":3,\"ok\":true}");
            println(json_kind(doc));
            println(json_get(doc, "name"));
            println(json_get(doc, "n"));
            println(json_get(doc, "ok"));
            println(json_get(doc, "missing"));
            json_set(doc, "name", "Lang");
            println(json_stringify(doc));
            var arr = json_parse("[1,2]");
            json_push(arr, 3);
            println(json_len(arr));
            println(json_index(arr, 2));
            var xml = xml_parse("<note id=\"1\">hi</note>");
            println(json_get(xml, "tag"));
            println(json_get(xml, "text"));
            println(json_get(json_get(xml, "attrs"), "id"));
            println(xml_stringify(xml));
            var y = yaml_parse("name: Terra\nn: 4\n");
            println(json_get(y, "name"));
            println(json_get(y, "n"));
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
            var root = path_join(os_tmp(), "terra-fs-" + to_string(os_pid()));
            fs_mkdir(root);
            var file = path_join(root, "note.txt");
            fs_write_text(file, "hi");
            fs_append_text(file, "!");
            println(fs_read_text(file));
            println(fs_exists(file));
            println(fs_is_file(file));
            println(fs_is_dir(root));
            println(fs_size(file));
            println(path_name(file));
            println(path_ext(file));
            var copy = path_join(root, "copy.txt");
            fs_copy(file, copy);
            var moved = path_join(root, "moved.txt");
            fs_rename(copy, moved);
            println(fs_read_text(moved));
            fs_write_base64(path_join(root, "b.txt"), b64_encode("Z"));
            println(fs_read_text(path_join(root, "b.txt")));
            println(fs_read_base64(file));
            println(fs_list(root));
            fs_remove(root);
            println(fs_exists(root));
            env_set("TERRA_STDLIB_PROBE", "yes");
            println(env_get("TERRA_STDLIB_PROBE"));
            println(env_has("TERRA_STDLIB_PROBE"));
            env_remove("TERRA_STDLIB_PROBE");
            println(env_has("TERRA_STDLIB_PROBE"));
            println(os_name());
            println(os_pid() > 0);
            println(list_len(os_args()));
            println(list_get(os_args(), 0));
            mut cmd: string = "sh";
            mut flag: string = "-c";
            if (os_name() == "windows") {
                cmd = "cmd";
                flag = "/c";
            }
            var pargs = list_new();
            list_push(pargs, flag);
            list_push(pargs, "echo hi");
            println(proc_output(cmd, pargs));
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
            var dbpath = path_join(os_tmp(), "terra-sql-" + to_string(os_pid()) + ".db");
            if (fs_exists(dbpath)) {
                fs_remove(dbpath);
            }
            var db = sqlite_open(dbpath);
            sqlite_exec(db, "create table notes(body text); insert into notes(body) values ('hello');");
            println(sqlite_query(db, "select body from notes"));
            sqlite_close(db);
            fs_remove(dbpath);
            var z = path_join(os_tmp(), "terra-zip-" + to_string(os_pid()) + ".zip");
            if (fs_exists(z)) {
                fs_remove(z);
            }
            zip_add(z, "a.txt", "alpha");
            zip_add(z, "b.txt", "beta");
            println(zip_list(z));
            println(zip_read(z, "a.txt"));
            var dest = path_join(os_tmp(), "terra-zip-out-" + to_string(os_pid()));
            if (fs_exists(dest)) {
                fs_remove(dest);
            }
            fs_mkdir(dest);
            zip_extract(z, dest);
            println(fs_read_text(path_join(dest, "b.txt")));
            fs_remove(z);
            fs_remove(dest);
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
            var sock = tcp_connect("127.0.0.1", to_int(data));
            tcp_send(sock, "ping");
            tcp_close(sock);
            return "sent";
        }
        func main(args: string[]) {
            var worker = thread_spawn("work", "terra");
            println(thread_join(worker));
            var listener = tcp_listen(0);
            var port = tcp_port(listener);
            var peer = thread_spawn("client", to_string(port));
            var sock = tcp_accept(listener);
            println(tcp_recv(sock, 16));
            tcp_close(sock);
            tcp_close(listener);
            println(thread_join(peer));
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
            http_serve_n(to_int(data), "page", 1);
            return "served";
        }
        func main(args: string[]) {
            try {
                http_get("https://example.com");
                println("https-yes");
            } exception (e: IoError) {
                println("https-no");
            }
            var listener = tcp_listen(0);
            var port = tcp_port(listener);
            tcp_close(listener);
            var worker = thread_spawn("serve", to_string(port));
            mut i: int = 0;
            mut body: string = "";
            while (i < 50) {
                try {
                    body = http_get("http://127.0.0.1:" + to_string(port) + "/ping");
                    i = 50;
                } exception (e: IoError) {
                    os_sleep(50);
                    i = i + 1;
                }
            }
            println(body);
            println(thread_join(worker));
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
    ];
    for (src, args, expect) in cases {
        let out = ok(run_args(src, args));
        assert!(out.contains(expect), "expected {expect} in {out}");
    }
}
