use terra_compiler::compile_file;
use terra_debugger::Debugger;
use terra_diagnostics::Lang;

fn session(src: &str, lang: Lang) -> Debugger {
    let compiled = compile_file("main.terra", src, lang).expect("compile");
    Debugger::new(compiled.module, &[], lang)
}

const SRC: &str = "func main(args: string[]) {\n    var x: int = 4;\n    println(x);\n}\n";

#[test]
fn breakpoint_vars_and_stack() {
    let mut dbg = session(SRC, Lang::En);
    let set = dbg.command("b main.terra:3").unwrap();
    assert!(set.contains("main.terra:3"), "{set}");
    let hit = dbg.command("run").unwrap();
    assert!(hit.contains("breakpoint"), "{hit}");
    assert!(hit.contains("main"), "{hit}");
    let value = dbg.command("p x").unwrap();
    assert!(value.contains("4"), "{value}");
    let stack = dbg.command("bt").unwrap();
    assert!(stack.contains("main"), "{stack}");
    let done = dbg.command("c").unwrap();
    assert!(done.contains("exited") || done.contains("0"), "{done}");
    assert!(dbg.stdout().contains('4'), "{}", dbg.stdout());
}

#[test]
fn step_into_and_russian() {
    let mut dbg = session(SRC, Lang::Ru);
    let hit = dbg.command("step").unwrap();
    assert!(hit.contains("main"), "{hit}");
    assert!(hit.contains("останов"), "{hit}");
    assert!(dbg.command("q").is_none());
}
