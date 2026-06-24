use std::cell::RefCell;
use std::rc::Rc;

use super::super::helpers::*;
use crate::object::{num_obj, str_obj, CallContext, HashData, Object};

pub(crate) fn markdown_module() -> Object {
    module(vec![
        ("parse", native("markdown.parse", markdown_parse)),
        (
            "renderTerminal",
            native("markdown.renderTerminal", markdown_render_terminal),
        ),
        ("fromHTML", native("markdown.fromHTML", markdown_from_html)),
    ])
}

pub(crate) fn markdown_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let source = match required_string(ctx, "markdown.parse", args, 0, "source") {
        Ok(v) => v,
        Err(e) => return e,
    };
    str_obj(source)
}

pub(crate) fn markdown_render_terminal(ctx: &mut CallContext, args: &[Object]) -> Object {
    let source = match required_string(ctx, "markdown.renderTerminal", args, 0, "source") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let width = match args.get(1) {
        Some(Object::Hash(_)) => {
            let w = hash_bool_arg(args.get(1), "width");
            let _ = w;
            match args.get(1) {
                Some(Object::Hash(h)) => match h.borrow().get("width") {
                    Some(Object::Number(n)) if *n >= 1.0 => *n as usize,
                    _ => 80,
                },
                _ => 80,
            }
        }
        _ => 80,
    };
    let normalized: String = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut out_lines: Vec<Object> = Vec::new();
    let mut headings: Vec<Object> = Vec::new();
    for line in &lines {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            out_lines.push(str_obj(format!("# {}", rest.trim())));
            headings.push(str_obj(rest.trim().to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("```") {
            out_lines.push(str_obj(format!("  {}", rest)));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            out_lines.push(str_obj(format!("- {}", &trimmed[2..])));
        } else if trimmed == "---" || trimmed == "***" {
            out_lines.push(str_obj("-".repeat(width)));
        } else if !trimmed.is_empty() {
            out_lines.push(str_obj(trimmed.to_string()));
        }
    }
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("lines", array(out_lines));
    hash.borrow_mut().set("width", num_obj(width as f64));
    hash.borrow_mut().set("headings", array(headings));
    hash.borrow_mut().set("links", array(Vec::new()));
    Object::Hash(hash)
}

pub(crate) fn markdown_from_html(ctx: &mut CallContext, args: &[Object]) -> Object {
    let html = match required_string(ctx, "markdown.fromHTML", args, 0, "html") {
        Ok(v) => v,
        Err(e) => return e,
    };
    str_obj(html_to_markdown(&html))
}

/// Minimal HTML-to-markdown: strip tags, preserve text, convert a few common
/// block elements to markdown equivalents.
fn html_to_markdown(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut in_tag = false;
    let mut tag = String::new();
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'<' {
            in_tag = true;
            tag.clear();
            i += 1;
            continue;
        }
        if c == b'>' {
            in_tag = false;
            let lower = tag.trim().to_lowercase();
            match lower.as_str() {
                "h1" | "h2" | "h3" => out.push_str("\n# "),
                "li" => out.push_str("\n- "),
                "p" | "br" | "div" => out.push('\n'),
                _ => {}
            }
            i += 1;
            continue;
        }
        if in_tag {
            tag.push(c as char);
        } else {
            out.push(c as char);
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// schema: JSON-Schema-style validate/assert.
// ---------------------------------------------------------------------------
