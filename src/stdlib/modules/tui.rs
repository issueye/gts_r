use std::cell::RefCell;
use std::io::{IsTerminal, Write};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};



use super::super::helpers::*;
use super::terminal::{terminal_cols, terminal_rows, terminal_size_object, terminal_style};
use super::text::{
    text_pad_to_width, text_strip_ansi, text_truncate_to_width, text_truncate_width,
    text_visible_chars, text_visible_width, text_width,
};
use crate::object::{
    bool_obj, new_error, num_obj, str_obj, Builtin,
    CallContext, HashData, Object,
};

#[derive(Clone)]
struct TuiApp {
    spec: Rc<RefCell<HashData>>,
    state: RefCell<Object>,
    running: std::cell::Cell<bool>,
    stopped: std::cell::Cell<bool>,
}

#[derive(Clone, Copy)]
struct TuiBoxOptions {
    width: i32,
    height: i32,
    padding: i32,
    border: bool,
}

#[derive(Clone)]
struct TuiInputOptions {
    title: String,
    value: String,
    cursor: i32,
    placeholder: String,
    prompt: String,
    width: i32,
    focused: bool,
    meta: String,
}

pub(crate) fn tui_module() -> Object {
    module(vec![
        ("createApp", native("tui.createApp", tui_create_app)),
        ("key", native("tui.key", tui_key)),
        ("text", native("tui.text", tui_text)),
        ("resize", native("tui.resize", tui_resize)),
        ("tick", native("tui.tick", tui_tick)),
        ("box", native("tui.box", tui_box)),
        ("input", native("tui.input", tui_input)),
        ("row", native("tui.row", tui_row)),
        ("column", native("tui.column", tui_column)),
        ("pad", native("tui.pad", tui_pad)),
        ("statusBar", native("tui.statusBar", tui_status_bar)),
        ("style", native("tui.style", terminal_style)),
        ("stripAnsi", native("tui.stripAnsi", text_strip_ansi)),
        ("width", native("tui.width", text_width)),
        ("truncate", native("tui.truncate", text_truncate_width)),
    ])
}

pub(crate) fn tui_create_app(ctx: &mut CallContext, args: &[Object]) -> Object {
    let spec = match args.first() {
        Some(Object::Hash(hash)) => hash.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.createApp: spec must be an object"),
        None => return new_error(ctx.pos.clone(), "tui.createApp requires spec"),
    };
    let app = Rc::new(TuiApp {
        spec: spec.clone(),
        state: RefCell::new(Object::Undefined),
        running: std::cell::Cell::new(false),
        stopped: std::cell::Cell::new(false),
    });
    if let Some(init_fn) = tui_hash_function(&spec.borrow(), "init") {
        let size = terminal_size_object();
        let result = call_script_function(&init_fn, ctx.env, &[size]);
        if result.is_runtime_error() {
            return result;
        }
        *app.state.borrow_mut() = result;
    } else if let Some(value) = spec.borrow().get("state").cloned() {
        *app.state.borrow_mut() = value;
    }
    tui_app_object(app)
}

pub(crate) fn tui_app_object(app: Rc<TuiApp>) -> Object {
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut()
        .set("__tuiApp", tui_app_marker(app.clone()));
    obj.borrow_mut().set(
        "dispatch",
        native_bound(
            "tui.app.dispatch",
            tui_app_dispatch,
            tui_app_marker(app.clone()),
        ),
    );
    obj.borrow_mut().set(
        "render",
        native_bound(
            "tui.app.render",
            tui_app_render,
            tui_app_marker(app.clone()),
        ),
    );
    obj.borrow_mut().set(
        "run",
        native_bound("tui.app.run", tui_app_run, tui_app_marker(app.clone())),
    );
    obj.borrow_mut().set(
        "stop",
        native_bound("tui.app.stop", tui_app_stop, tui_app_marker(app.clone())),
    );
    obj.borrow_mut().set(
        "state",
        native_bound("tui.app.state", tui_app_state, tui_app_marker(app)),
    );
    Object::Hash(obj)
}

pub(crate) fn tui_app_marker(app: Rc<TuiApp>) -> Object {
    let marker = Rc::new(RefCell::new(HashData::default()));
    marker.borrow_mut().set("__kind", str_obj("tuiApp"));
    marker
        .borrow_mut()
        .set("__ptr", str_obj(format!("{:p}", Rc::as_ptr(&app))));
    TUI_APPS.with(|apps| apps.borrow_mut().push(app));
    Object::Hash(marker)
}

thread_local! {
    static TUI_APPS: RefCell<Vec<Rc<TuiApp>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn native_bound(
    name: &str,
    func: impl Fn(&mut CallContext<'_>, &[Object]) -> Object + 'static,
    extra: Object,
) -> Object {
    Object::Builtin(Rc::new(Builtin {
        name: name.into(),
        func: Rc::new(func),
        extra: Some(extra),
    }))
}

pub(crate) fn bound_tui_app(ctx: &CallContext, name: &str) -> Result<Rc<TuiApp>, Object> {
    let Some(Object::Hash(marker)) = ctx.receiver.clone() else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{name}: missing app receiver"),
        ));
    };
    let ptr = match marker.borrow().get("__ptr") {
        Some(Object::String(value)) => value.to_string(),
        _ => {
            return Err(new_error(
                ctx.pos.clone(),
                format!("{name}: invalid app receiver"),
            ))
        }
    };
    TUI_APPS.with(|apps| {
        apps.borrow()
            .iter()
            .find(|app| format!("{:p}", Rc::as_ptr(app)) == ptr)
            .cloned()
            .ok_or_else(|| new_error(ctx.pos.clone(), format!("{name}: invalid app receiver")))
    })
}

pub(crate) fn tui_app_dispatch(ctx: &mut CallContext, args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.dispatch") {
        Ok(app) => app,
        Err(err) => return err,
    };
    let msg = args.first().cloned().unwrap_or(Object::Undefined);
    match tui_app_do_dispatch(ctx, &app, msg) {
        Ok(()) => app.state.borrow().clone(),
        Err(err) => err,
    }
}

pub(crate) fn tui_app_render(ctx: &mut CallContext, args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.render") {
        Ok(app) => app,
        Err(err) => return err,
    };
    let size = match args.first() {
        Some(Object::Hash(hash)) => Object::Hash(hash.clone()),
        Some(Object::Null | Object::Undefined) | None => terminal_size_object(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.app.render: size must be an object"),
    };
    match tui_app_do_render(ctx, &app, size) {
        Ok(frame) => str_obj(frame),
        Err(err) => err,
    }
}

pub(crate) fn tui_app_run(ctx: &mut CallContext, args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.run") {
        Ok(app) => app,
        Err(err) => return err,
    };
    if app.running.get() {
        return new_error(ctx.pos.clone(), "tui.app.run: app is already running");
    }
    if let Some(arg) = args.first() {
        if !matches!(arg, Object::Hash(_) | Object::Null | Object::Undefined) {
            return new_error(ctx.pos.clone(), "tui.app.run: options must be an object");
        }
    }
    let opts = args.first().and_then(|arg| match arg {
        Object::Hash(hash) => Some(hash.clone()),
        _ => None,
    });
    let tick_ms = opts
        .as_ref()
        .and_then(|hash| tui_hash_number(&hash.borrow(), "tickMs"))
        .filter(|value| *value > 0.0)
        .unwrap_or(120.0) as u64;
    let alternate_screen = opts
        .as_ref()
        .and_then(|hash| tui_hash_bool(&hash.borrow(), "alternateScreen"))
        .unwrap_or(false);
    let hide_cursor = opts
        .as_ref()
        .and_then(|hash| tui_hash_bool(&hash.borrow(), "hideCursor"))
        .unwrap_or(false);
    app.running.set(true);
    app.stopped.set(false);
    let result = tui_app_run_loop(ctx, &app, tick_ms, alternate_screen, hide_cursor);
    app.running.set(false);
    match result {
        Ok(()) => app.state.borrow().clone(),
        Err(err) => err,
    }
}

pub(crate) fn tui_app_stop(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.stop") {
        Ok(app) => app,
        Err(err) => return err,
    };
    app.stopped.set(true);
    Object::Undefined
}

pub(crate) fn tui_app_state(ctx: &mut CallContext, _args: &[Object]) -> Object {
    match bound_tui_app(ctx, "tui.app.state") {
        Ok(app) => app.state.borrow().clone(),
        Err(err) => err,
    }
}

fn tui_app_run_loop(
    ctx: &mut CallContext,
    app: &Rc<TuiApp>,
    tick_ms: u64,
    alternate_screen: bool,
    hide_cursor: bool,
) -> Result<(), Object> {
    let mut stdout = std::io::stdout();
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if !interactive {
        tui_app_do_dispatch(
            ctx,
            app,
            tui_resize_message(terminal_cols(), terminal_rows(), true),
        )?;
        tui_app_render_to_stdout(ctx, app)?;
        return Ok(());
    }

    use crossterm::{
        cursor::{Hide, Show},
        event::{self, Event, KeyEventKind},
        execute,
        terminal::{
            self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
        },
    };

    if enable_raw_mode().is_err() {
        return Err(new_error(
            ctx.pos.clone(),
            "tui.app.run: failed to enable raw mode",
        ));
    }
    if alternate_screen {
        let _ = execute!(stdout, EnterAlternateScreen);
    }
    if hide_cursor {
        let _ = execute!(stdout, Hide);
    }

    let mut last_size =
        terminal::size().unwrap_or((terminal_cols() as u16, terminal_rows() as u16));
    let _ = tui_app_do_dispatch(
        ctx,
        app,
        tui_resize_message(last_size.0 as i32, last_size.1 as i32, true),
    );
    let mut result = tui_app_render_to_stdout(ctx, app);
    let mut next_tick = Instant::now() + Duration::from_millis(tick_ms.max(1));

    while result.is_ok() && !app.stopped.get() {
        let now = Instant::now();
        let timeout = next_tick.saturating_duration_since(now);
        match event::poll(timeout) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) => {
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                        result = tui_app_do_dispatch(ctx, app, tui_key_event_message(key))
                            .and_then(|_| tui_app_render_to_stdout(ctx, app));
                    }
                }
                Ok(Event::Paste(text)) => {
                    result = tui_app_do_dispatch(ctx, app, tui_raw_message(text))
                        .and_then(|_| tui_app_render_to_stdout(ctx, app));
                }
                Ok(Event::Resize(cols, rows)) => {
                    last_size = (cols, rows);
                    result = tui_app_do_dispatch(
                        ctx,
                        app,
                        tui_resize_message(cols as i32, rows as i32, true),
                    )
                    .and_then(|_| tui_app_render_to_stdout(ctx, app));
                }
                Ok(Event::Mouse(mouse)) => {
                    result = tui_app_do_dispatch(ctx, app, tui_mouse_event_message(mouse))
                        .and_then(|_| tui_app_render_to_stdout(ctx, app));
                }
                Ok(_) => {}
                Err(err) => {
                    result = Err(new_error(ctx.pos.clone(), format!("tui.app.run: {}", err)));
                }
            },
            Ok(false) => {
                next_tick = Instant::now() + Duration::from_millis(tick_ms.max(1));
                if let Ok(size) = terminal::size() {
                    if size != last_size {
                        last_size = size;
                        result = tui_app_do_dispatch(
                            ctx,
                            app,
                            tui_resize_message(size.0 as i32, size.1 as i32, true),
                        );
                    }
                }
                if result.is_ok() {
                    result = tui_app_do_dispatch(ctx, app, tui_tick_message())
                        .and_then(|_| tui_app_render_to_stdout(ctx, app));
                }
            }
            Err(err) => result = Err(new_error(ctx.pos.clone(), format!("tui.app.run: {}", err))),
        }
    }

    if hide_cursor {
        let _ = execute!(stdout, Show);
    }
    if alternate_screen {
        let _ = execute!(stdout, LeaveAlternateScreen);
    }
    let _ = disable_raw_mode();
    result
}

fn tui_app_render_to_stdout(ctx: &mut CallContext, app: &Rc<TuiApp>) -> Result<(), Object> {
    let frame = tui_app_do_render(ctx, app, terminal_size_object())?;
    std::io::stdout()
        .write_all(format!("\x1b[H{}", frame).as_bytes())
        .map_err(|err| new_error(ctx.pos.clone(), format!("tui.app.render: {}", err)))?;
    let _ = std::io::stdout().flush();
    Ok(())
}

pub(crate) fn tui_app_do_dispatch(
    ctx: &mut CallContext,
    app: &Rc<TuiApp>,
    msg: Object,
) -> Result<(), Object> {
    if let Some(update_fn) = tui_hash_function(&app.spec.borrow(), "update") {
        let state = app.state.borrow().clone();
        let result = call_script_function(&update_fn, ctx.env, &[state, msg]);
        if result.is_runtime_error() {
            return Err(result);
        }
        if let Object::Hash(hash) = &result {
            if let Some(next) = hash.borrow().get("state").cloned() {
                *app.state.borrow_mut() = next;
            } else {
                *app.state.borrow_mut() = result.clone();
            }
            if tui_hash_bool(&hash.borrow(), "quit").unwrap_or(false) {
                app.stopped.set(true);
            }
        } else {
            *app.state.borrow_mut() = result;
        }
    } else if let Object::Hash(hash) = msg {
        if tui_hash_string(&hash.borrow(), "type").as_deref() == Some("quit") {
            app.stopped.set(true);
        }
    }
    Ok(())
}

pub(crate) fn tui_app_do_render(
    ctx: &mut CallContext,
    app: &Rc<TuiApp>,
    size: Object,
) -> Result<String, Object> {
    if let Some(view_fn) = tui_hash_function(&app.spec.borrow(), "view") {
        let state = app.state.borrow().clone();
        let result = call_script_function(&view_fn, ctx.env, &[state, size]);
        if result.is_runtime_error() {
            return Err(result);
        }
        Ok(tui_frame_text(&result))
    } else {
        Ok(value_to_string(&app.state.borrow()))
    }
}

pub(crate) fn tui_key(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "tui.key", args, 0, "name") {
        Ok(name) => name,
        Err(err) => return err,
    };
    let msg = tui_key_message(&name, "");
    if let Some(raw) = args.get(1) {
        if let Object::Hash(hash) = &msg {
            hash.borrow_mut().set("raw", str_obj(value_to_string(raw)));
        }
    }
    msg
}

pub(crate) fn tui_text(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "tui.text", args, 0, "value") {
        Ok(value) => value,
        Err(err) => return err,
    };
    tui_text_message(&value, &value)
}

pub(crate) fn tui_resize(ctx: &mut CallContext, args: &[Object]) -> Object {
    let cols = match required_number(ctx, "tui.resize", args, 0, "cols") {
        Ok(cols) => cols,
        Err(err) => return err,
    };
    let rows = match required_number(ctx, "tui.resize", args, 1, "rows") {
        Ok(rows) => rows,
        Err(err) => return err,
    };
    tui_resize_message(cols as i32, rows as i32, true)
}

pub(crate) fn tui_tick(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    tui_tick_message()
}

pub(crate) fn tui_key_message(name: &str, raw: &str) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("key"));
    hash.borrow_mut().set("key", str_obj(name));
    if !raw.is_empty() {
        hash.borrow_mut().set("raw", str_obj(raw));
    }
    Object::Hash(hash)
}

pub(crate) fn tui_text_message(value: &str, raw: &str) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("text"));
    hash.borrow_mut().set("text", str_obj(value));
    if !raw.is_empty() {
        hash.borrow_mut().set("raw", str_obj(raw));
    }
    Object::Hash(hash)
}

pub(crate) fn tui_resize_message(cols: i32, rows: i32, stable: bool) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("resize"));
    hash.borrow_mut().set("cols", num_obj(cols as f64));
    hash.borrow_mut().set("rows", num_obj(rows as f64));
    hash.borrow_mut().set("stable", bool_obj(stable));
    Object::Hash(hash)
}

pub(crate) fn tui_tick_message() -> Object {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("tick"));
    hash.borrow_mut().set("timeMs", num_obj(ms));
    Object::Hash(hash)
}

fn tui_raw_message(raw: String) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("raw"));
    hash.borrow_mut().set("raw", str_obj(raw));
    Object::Hash(hash)
}

fn tui_key_event_message(event: crossterm::event::KeyEvent) -> Object {
    use crossterm::event::{KeyCode, KeyModifiers};

    if event.modifiers.contains(KeyModifiers::CONTROL) {
        let name = match event.code {
            KeyCode::Char('c') | KeyCode::Char('C') => "ctrl+c",
            KeyCode::Char('o') | KeyCode::Char('O') => "ctrl+o",
            KeyCode::Char('q') | KeyCode::Char('Q') => "ctrl+q",
            KeyCode::Char('r') | KeyCode::Char('R') => "ctrl+r",
            KeyCode::Char('s') | KeyCode::Char('S') => "ctrl+s",
            _ => "",
        };
        if !name.is_empty() {
            return tui_key_message(name, "");
        }
    }

    match event.code {
        KeyCode::Backspace => tui_key_message("backspace", ""),
        KeyCode::Enter => tui_key_message("enter", ""),
        KeyCode::Left => tui_key_message("left", ""),
        KeyCode::Right => tui_key_message("right", ""),
        KeyCode::Up => tui_key_message("up", ""),
        KeyCode::Down => tui_key_message("down", ""),
        KeyCode::Home => tui_key_message("home", ""),
        KeyCode::End => tui_key_message("end", ""),
        KeyCode::PageUp => tui_key_message("pageUp", ""),
        KeyCode::PageDown => tui_key_message("pageDown", ""),
        KeyCode::Tab => tui_key_message("tab", ""),
        KeyCode::BackTab => tui_key_message("shift+tab", ""),
        KeyCode::Esc => tui_key_message("escape", ""),
        KeyCode::Char(ch) => tui_text_message(&ch.to_string(), &ch.to_string()),
        _ => tui_key_message("unknown", ""),
    }
}

fn tui_mouse_event_message(event: crossterm::event::MouseEvent) -> Object {
    use crossterm::event::MouseEventKind;

    let (action, button) = match event.kind {
        MouseEventKind::Down(button) => ("down", tui_mouse_button_number(button)),
        MouseEventKind::Up(button) => ("release", tui_mouse_button_number(button)),
        MouseEventKind::Drag(button) => ("drag", tui_mouse_button_number(button)),
        MouseEventKind::Moved => ("move", 0),
        MouseEventKind::ScrollUp => ("wheel", 64),
        MouseEventKind::ScrollDown => ("wheel", 65),
        MouseEventKind::ScrollLeft => ("wheelLeft", 66),
        MouseEventKind::ScrollRight => ("wheelRight", 67),
    };
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("mouse"));
    hash.borrow_mut().set("action", str_obj(action));
    hash.borrow_mut().set("button", num_obj(button as f64));
    hash.borrow_mut().set("x", num_obj(event.column as f64));
    hash.borrow_mut().set("y", num_obj(event.row as f64));
    Object::Hash(hash)
}

fn tui_mouse_button_number(button: crossterm::event::MouseButton) -> i32 {
    use crossterm::event::MouseButton;
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 2,
        MouseButton::Middle => 1,
    }
}

pub(crate) fn tui_box(ctx: &mut CallContext, args: &[Object]) -> Object {
    let content = args.first().map(tui_frame_text).unwrap_or_default();
    let mut title = String::new();
    let mut opts = TuiBoxOptions {
        width: 0,
        height: 0,
        padding: 0,
        border: true,
    };
    if let Some(arg) = args.get(1) {
        if !matches!(arg, Object::Null | Object::Undefined) {
            let Object::Hash(hash) = arg else {
                return new_error(ctx.pos.clone(), "tui.box: options must be an object");
            };
            let hash = hash.borrow();
            title = tui_hash_string(&hash, "title").unwrap_or_default();
            match tui_hash_int_option(ctx, "tui.box", &hash, "width") {
                Ok(Some(n)) => opts.width = n,
                Ok(None) => {}
                Err(err) => return err,
            }
            match tui_hash_int_option(ctx, "tui.box", &hash, "height") {
                Ok(Some(n)) => opts.height = n,
                Ok(None) => {}
                Err(err) => return err,
            }
            match tui_hash_int_option(ctx, "tui.box", &hash, "padding") {
                Ok(Some(n)) => opts.padding = n,
                Ok(None) => {}
                Err(err) => return err,
            }
            if let Some(border) = tui_hash_bool(&hash, "border") {
                opts.border = border;
            }
        }
    }
    str_obj(render_tui_box(&content, &title, opts))
}

pub(crate) fn render_tui_box(content: &str, title: &str, opts: TuiBoxOptions) -> String {
    let normalized = content.replace("\r\n", "\n");
    let mut lines: Vec<String> = if normalized.is_empty() {
        Vec::new()
    } else {
        normalized.split('\n').map(|s| s.to_string()).collect()
    };
    let mut inner_width = lines
        .iter()
        .map(|line| text_visible_width(line))
        .max()
        .unwrap_or(0);
    if !title.is_empty() {
        inner_width = inner_width.max(text_visible_width(title) + 2);
    }
    let padding = opts.padding.max(0) as usize;
    if opts.width > 0 {
        let mut target = opts.width as isize;
        if opts.border {
            target -= 2;
        }
        target -= (padding * 2) as isize;
        inner_width = target.max(0) as usize;
    }
    let pad = " ".repeat(padding);
    let blank = format!("{}{}{}", pad, text_pad_to_width("", inner_width), pad);
    let mut body = Vec::new();
    for _ in 0..padding {
        body.push(blank.clone());
    }
    for line in lines.drain(..) {
        body.push(format!(
            "{}{}{}",
            pad,
            text_pad_to_width(&text_truncate_to_width(&line, inner_width), inner_width),
            pad
        ));
    }
    for _ in 0..padding {
        body.push(blank.clone());
    }
    if opts.height > 0 {
        let target = if opts.border {
            opts.height - 2
        } else {
            opts.height
        }
        .max(0) as usize;
        while body.len() < target {
            body.push(blank.clone());
        }
        body.truncate(target);
    }
    if !opts.border {
        return body.join("\n");
    }
    let width = inner_width + padding * 2;
    let title_text = if title.is_empty() {
        String::new()
    } else {
        format!(
            " {} ",
            text_truncate_to_width(title, width.saturating_sub(2))
        )
    };
    let top_fill = width.saturating_sub(text_visible_width(&title_text));
    let mut out = vec![format!("┌{}{}┐", title_text, "─".repeat(top_fill))];
    for line in body {
        out.push(format!("│{}│", text_pad_to_width(&line, width)));
    }
    out.push(format!("└{}┘", "─".repeat(width)));
    out.join("\n")
}

pub(crate) fn tui_input(ctx: &mut CallContext, args: &[Object]) -> Object {
    let hash = match args.first() {
        Some(Object::Hash(hash)) => hash.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.input: options must be an object"),
        None => return new_error(ctx.pos.clone(), "tui.input requires options"),
    };
    let hash = hash.borrow();
    let mut opts = TuiInputOptions {
        title: tui_hash_string(&hash, "title").unwrap_or_else(|| "Input".into()),
        value: tui_hash_string(&hash, "value").unwrap_or_default(),
        cursor: 0,
        placeholder: tui_hash_string(&hash, "placeholder").unwrap_or_default(),
        prompt: tui_hash_string(&hash, "prompt").unwrap_or_else(|| "> ".into()),
        width: 80,
        focused: tui_hash_bool(&hash, "focused").unwrap_or(true),
        meta: tui_hash_string(&hash, "meta").unwrap_or_default(),
    };
    opts.cursor = text_visible_chars(&opts.value).len() as i32;
    match tui_hash_int_option(ctx, "tui.input", &hash, "cursor") {
        Ok(Some(cursor)) => opts.cursor = cursor,
        Ok(None) => {}
        Err(err) => return err,
    }
    match tui_hash_int_option(ctx, "tui.input", &hash, "width") {
        Ok(Some(width)) => opts.width = width,
        Ok(None) => {}
        Err(err) => return err,
    }
    opts.width = opts.width.max(1);
    opts.cursor = opts
        .cursor
        .clamp(0, text_visible_chars(&opts.value).len() as i32);
    str_obj(render_tui_input(&opts))
}

pub(crate) fn render_tui_input(opts: &TuiInputOptions) -> String {
    let width = opts.width.max(1) as usize;
    let input_width = width
        .saturating_sub(text_visible_width(&opts.prompt))
        .max(1);
    let mut lines = vec![
        terminal_style_string(&text_pad_to_width(&opts.title, width), "accent", true),
        format!(
            "{}{}",
            opts.prompt,
            render_tui_input_value(opts, input_width)
        ),
    ];
    if !opts.meta.is_empty() {
        lines.push(terminal_style_string(
            &text_pad_to_width(&opts.meta, width),
            "muted",
            false,
        ));
    }
    lines.join("\n")
}

pub(crate) fn render_tui_input_value(opts: &TuiInputOptions, width: usize) -> String {
    if opts.value.is_empty() && !opts.placeholder.is_empty() {
        return terminal_style_string(
            &text_pad_to_width(&text_truncate_to_width(&opts.placeholder, width), width),
            "muted",
            false,
        );
    }
    if !opts.focused {
        return text_pad_to_width(&text_truncate_to_width(&opts.value, width), width);
    }
    crop_tui_input_around_cursor(&opts.value, opts.cursor, width)
}

pub(crate) fn crop_tui_input_around_cursor(value: &str, cursor: i32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars = text_visible_chars(value);
    let cursor = (cursor.max(0) as usize).min(chars.len());
    let before_budget = (width - 1) * 62 / 100;
    let after_budget = width - 1 - before_budget;
    let mut before = Vec::new();
    let mut before_width = 0usize;
    for ch in chars[..cursor].iter().rev() {
        let next = text_visible_width(ch);
        if before_width + next > before_budget {
            break;
        }
        before.push(ch.clone());
        before_width += next;
    }
    before.reverse();
    let mut after = Vec::new();
    let mut after_width = 0usize;
    for ch in &chars[cursor..] {
        let next = text_visible_width(ch);
        if after_width + next > after_budget {
            break;
        }
        after.push(ch.clone());
        after_width += next;
    }
    let row = format!("{}\x1b[7m \x1b[0m{}", before.join(""), after.join(""));
    text_pad_to_width(&row, width)
}

pub(crate) fn tui_row(ctx: &mut CallContext, args: &[Object]) -> Object {
    match tui_layout_parts(args) {
        Ok(parts) => str_obj(join_tui_horizontal(&parts)),
        Err(err) => new_error(ctx.pos.clone(), err),
    }
}

pub(crate) fn tui_column(ctx: &mut CallContext, args: &[Object]) -> Object {
    match tui_layout_parts(args) {
        Ok(parts) => str_obj(parts.join("\n")),
        Err(err) => new_error(ctx.pos.clone(), err),
    }
}

pub(crate) fn tui_pad(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(content) = args.first() else {
        return new_error(ctx.pos.clone(), "tui.pad requires content");
    };
    let content = tui_frame_text(content);
    let padding = match args.get(1) {
        Some(Object::Number(n)) => (*n as i32).max(0) as usize,
        Some(_) => return new_error(ctx.pos.clone(), "tui.pad: padding must be a number"),
        None => 1,
    };
    let prefix = " ".repeat(padding);
    let mut lines: Vec<String> = content
        .replace("\r\n", "\n")
        .split('\n')
        .map(|line| format!("{}{}{}", prefix, line, prefix))
        .collect();
    let blank_width = lines
        .iter()
        .map(|line| text_visible_width(line))
        .max()
        .unwrap_or(0);
    let blank = " ".repeat(blank_width);
    for _ in 0..padding {
        lines.insert(0, blank.clone());
        lines.push(blank.clone());
    }
    str_obj(lines.join("\n"))
}

pub(crate) fn tui_status_bar(ctx: &mut CallContext, args: &[Object]) -> Object {
    let hash = match args.first() {
        Some(Object::Hash(hash)) => hash.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.statusBar: parts must be an object"),
        None => return new_error(ctx.pos.clone(), "tui.statusBar requires parts"),
    };
    let width = match args.get(1) {
        Some(Object::Number(n)) => (*n as i32).max(1) as usize,
        Some(_) => return new_error(ctx.pos.clone(), "tui.statusBar: width must be a number"),
        None => 80,
    };
    let hash = hash.borrow();
    let mut left = tui_hash_string(&hash, "left").unwrap_or_default();
    let center = tui_hash_string(&hash, "center").unwrap_or_default();
    let mut right = tui_hash_string(&hash, "right").unwrap_or_default();
    left = text_truncate_to_width(&left, width);
    let right_budget = width
        .saturating_sub(text_visible_width(&left))
        .saturating_sub(1);
    right = text_truncate_to_width(&right, right_budget);
    let mut line = left;
    if !center.is_empty()
        && width
            >= text_visible_width(&line)
                + text_visible_width(&right)
                + text_visible_width(&center)
                + 2
    {
        let center_pos = (width - text_visible_width(&center)) / 2;
        line = format!("{}{}", text_pad_to_width(&line, center_pos), center);
    }
    line = format!(
        "{}{}",
        text_pad_to_width(&line, width.saturating_sub(text_visible_width(&right))),
        right
    );
    str_obj(text_truncate_to_width(&line, width))
}

pub(crate) fn tui_layout_parts(args: &[Object]) -> Result<Vec<String>, String> {
    if args.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(Object::Array(arr)) = args.first() {
        return Ok(arr.borrow().elements.iter().map(tui_frame_text).collect());
    }
    Ok(args.iter().map(tui_frame_text).collect())
}

pub(crate) fn join_tui_horizontal(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let blocks: Vec<Vec<String>> = parts
        .iter()
        .map(|part| {
            part.replace("\r\n", "\n")
                .split('\n')
                .map(|s| s.to_string())
                .collect()
        })
        .collect();
    let height = blocks.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = blocks
        .iter()
        .map(|lines| {
            lines
                .iter()
                .map(|line| text_visible_width(line))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = Vec::with_capacity(height);
    for row in 0..height {
        let mut line = String::new();
        for (col, lines) in blocks.iter().enumerate() {
            let part = lines.get(row).map(String::as_str).unwrap_or("");
            line.push_str(&text_pad_to_width(part, widths[col]));
        }
        out.push(line);
    }
    out.join("\n")
}

pub(crate) fn tui_frame_text(value: &Object) -> String {
    if let Object::Array(arr) = value {
        return arr
            .borrow()
            .elements
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join("\n");
    }
    value_to_string(value)
}

pub(crate) fn tui_hash_function(hash: &HashData, key: &str) -> Option<Object> {
    match hash.get(key) {
        Some(Object::Function(_) | Object::Builtin(_) | Object::Closure(_)) => {
            hash.get(key).cloned()
        }
        _ => None,
    }
}

pub(crate) fn tui_hash_string(hash: &HashData, key: &str) -> Option<String> {
    match hash.get(key) {
        Some(Object::String(value)) => Some(value.to_string()),
        Some(Object::Null | Object::Undefined) | None => None,
        Some(value) => Some(value_to_string(value)),
    }
}

pub(crate) fn tui_hash_bool(hash: &HashData, key: &str) -> Option<bool> {
    match hash.get(key) {
        Some(Object::Boolean(value)) => Some(*value),
        Some(Object::Null | Object::Undefined) | None => None,
        Some(value) => Some(value.is_truthy()),
    }
}

pub(crate) fn tui_hash_number(hash: &HashData, key: &str) -> Option<f64> {
    match hash.get(key) {
        Some(Object::Number(value)) => Some(*value),
        _ => None,
    }
}

pub(crate) fn tui_hash_int_option(
    ctx: &CallContext,
    name: &str,
    hash: &HashData,
    key: &str,
) -> Result<Option<i32>, Object> {
    match hash.get(key) {
        Some(Object::Number(n)) => Ok(Some(*n as i32)),
        Some(Object::Null | Object::Undefined) | None => Ok(None),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!("{name}: {key} must be a number"),
        )),
    }
}
