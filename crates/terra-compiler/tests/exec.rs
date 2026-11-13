use terra_compiler::compile_file;
use terra_diagnostics::Lang;
use terra_runtime::execute;

fn run(src: &str) -> terra_runtime::Outcome {
    let compiled = compile_file("main.terra", src, Lang::En).unwrap_or_else(|e| {
        panic!("{}", e.messages.join("\n"));
    });
    execute(&compiled.module, &[], Lang::En, false)
}

#[test]
fn hello() {
    let out = run(
        r#"
        func main(args: string[]) {
            println("Hello, Terra");
        }
        "#,
    );
    assert_eq!(out.exit, 0, "{}", out.stderr);
    assert_eq!(out.stdout, "Hello, Terra\n");
    assert!(out.stderr.is_empty(), "{}", out.stderr);
}

#[test]
fn returns_int() {
    let out = run(
        r#"
        func main(args: string[]) -> int {
            return 7;
        }
        "#,
    );
    assert_eq!(out.exit, 7, "{}", out.stderr);
}

#[test]
fn missing_semicolon_is_an_error() {
    let err = compile_file(
        "main.terra",
        "func main(args: string[]) { println(\"x\") }",
        Lang::En,
    )
    .unwrap_err();
    let text = err.messages.join("\n");
    assert!(text.contains("E001") || text.to_lowercase().contains("semicolon"), "{text}");
}

#[test]
fn missing_main() {
    let err = compile_file("main.terra", "func other() { println(\"x\"); }", Lang::En).unwrap_err();
    let text = err.messages.join("\n");
    assert!(text.to_lowercase().contains("main"), "{text}");
}

#[test]
fn immutable_binding() {
    let err = compile_file(
        "main.terra",
        r#"
        func main(args: string[]) {
            var x: int = 1;
            x = 2;
        }
        "#,
        Lang::En,
    )
    .unwrap_err();
    let text = err.messages.join("\n");
    assert!(text.to_lowercase().contains("x"), "{text}");
}

#[test]
fn class_and_stdlib() {
    let out = run(
        r#"
        func main(args: string[]) {
            var p = new Pair(2, 3);
            println(p.sum());
            println(abs(0 - 4));
            delete p;
        }
        "#,
    );
    assert_eq!(out.stdout, "5\n4\n", "{}", out.stderr);
    assert_eq!(out.exit, 0);
    assert!(out.stderr.is_empty(), "{}", out.stderr);
}

#[test]
fn exception_and_divzero() {
    let out = run(
        r#"
        func main(args: string[]) {
            try {
                var x: int = 1 / 0;
                println(x);
            } exception (e: DivZero) {
                println(e.message);
            }
        }
        "#,
    );
    assert_eq!(out.stdout, "division by zero\n", "{}", out.stderr);
    assert_eq!(out.exit, 0);
}

#[test]
fn async_await() {
    let out = run(
        r#"
        async func value() -> int {
            return 7;
        }
        func main(args: string[]) {
            println(await value());
        }
        "#,
    );
    assert_eq!(out.stdout, "7\n", "{}", out.stderr);
    assert_eq!(out.exit, 0, "{}", out.stderr);
}

#[test]
fn loops_match_and_array() {
    let out = run(
        r#"
        func main(args: string[]) {
            var nums = [1, 2, 3];
            mut sum: int = 0;
            for (n in nums) {
                sum = sum + n;
            }
            for (i in 0..3) {
                sum = sum + i;
            }
            match (sum) {
                9 => { println("nine"); }
                _ => { println("other"); }
            }
            delete nums;
        }
        "#,
    );
    assert_eq!(out.stdout, "nine\n", "{}", out.stderr);
}

#[test]
fn class_entry_override_and_interface() {
    let out = run(
        r#"
        interface Speak {
            func say() -> string;
        }
        pub class Animal {
            pub func say() -> string {
                return "animal";
            }
        }
        pub class Dog extends Animal implements Speak {
            pub override func say() -> string {
                return "woof";
            }
            pub func parent() -> string {
                return super.say();
            }
        }
        func talk(s: Speak) {
            println(s.say());
        }
        pub class File {
            func main(args: string[]) {
                var dog = new Dog();
                println(dog.say());
                println(dog.parent());
                talk(dog);
                delete dog;
            }
        }
        "#,
    );
    assert_eq!(out.stdout, "woof\nanimal\nwoof\n", "{}", out.stderr);
    assert_eq!(out.exit, 0);
}

#[test]
fn use_after_free_and_enum() {
    let out = run(
        r#"
        enum Color { Red, Blue }
        func main(args: string[]) {
            var c = Color.Red;
            match (c) {
                Color.Red => { println("red"); }
                Color.Blue => { println("blue"); }
            }
            var p = new Pair(1, 2);
            delete p;
            println(p.first);
        }
        "#,
    );
    assert_eq!(out.exit, 1, "{}", out.stdout);
    assert!(out.stdout.contains("red\n"), "{}", out.stdout);
    assert!(out.stderr.to_lowercase().contains("use after free"), "{}", out.stderr);
}

#[test]
fn package_use() {
    let compiled = terra_compiler::compile(
        &[
            terra_compiler::Unit {
                name: "src/com/example/util/Util.terra".into(),
                package: Some("com.example.util".into()),
                source: "package com.example.util;\n\npub func hi() -> string {\n    return \"hi\";\n}\n".into(),
            },
            terra_compiler::Unit {
                name: "src/com/example/app/App.terra".into(),
                package: Some("com.example.app".into()),
                source: "package com.example.app;\n\nuse com.example.util;\n\nfunc main(args: string[]) {\n    println(hi());\n}\n".into(),
            },
        ],
        Lang::En,
    )
    .unwrap_or_else(|e| panic!("{}", e.messages.join("\n")));
    let out = execute(&compiled.module, &[], Lang::En, false);
    assert_eq!(out.stdout, "hi\n", "{}", out.stderr);
}
