use gts::lexer::Lexer;
fn main() {
    let src = "0..60 =>";
    let mut lex = Lexer::new(src);
    let mut n = 0;
    loop {
        let t = lex.next_token();
        println!("{:?}", t.kind);
        n += 1;
        if t.kind == gts::lexer::TokenKind::Eof || n > 12 {
            break;
        }
    }
}
