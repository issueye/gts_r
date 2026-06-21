use gts::lexer::Lexer;
use gts::parser::Parser;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut ok = 0usize;
    let mut fail = 0usize;
    let files: Vec<String> = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        vec![]
    };
    for f in &files {
        let src = match fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("READ FAIL {}: {}", f, e);
                fail += 1;
                continue;
            }
        };
        let lex = Lexer::new(&src);
        let mut p = Parser::new(lex, f);
        let prog = p.parse_program();
        if !prog.errors.is_empty() {
            eprintln!("PARSE FAIL {}: {} error(s)", f, prog.errors.len());
            for e in prog.errors.iter().take(5) {
                eprintln!("    {}", e);
            }
            fail += 1;
        } else {
            println!("OK {} ({} stmts)", f, prog.body.len());
            ok += 1;
        }
    }
    println!("\n{} ok, {} fail", ok, fail);
}
