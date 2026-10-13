// Stand-in C compiler for `cargo check --target *-apple-darwin` on Windows: writes an empty file wherever
// `-o` points and succeeds. Nothing is linked during a check, so the empty object is never used.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "-o") {
        if let Some(out) = args.get(i + 1) {
            let _ = std::fs::write(out, b"");
        }
    }
}
