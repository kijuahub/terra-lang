//! Slim launcher. A built program is this executable plus a Terra trailer.
//! The `terra` CLI uses the same trailer check, so `terra build` can copy itself.

fn main() {
    let code = run().unwrap_or_else(|err| {
        eprintln!("{err}");
        1
    });
    std::process::exit(code);
}

fn run() -> Result<i32, String> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let payload = terra_bytecode::read_embedded(&bytes)
        .ok_or_else(|| "this executable has no Terra program embedded".to_string())?;
    let module = terra_bytecode::decode(&payload)?;
    let lang = terra_runtime::lang_of(&module);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = terra_runtime::execute(&module, &args, lang, true);
    Ok(outcome.exit)
}
