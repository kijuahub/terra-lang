//! Terra UI. Views are host values. `ui.serve` turns the view a function returns into a page.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use serde_json::{Map, Value as Json};

use crate::hostcall::{read_http, HostObj};
use crate::{Fault, SlotData, Value, Vm};

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Empty,
    El {
        tag: &'static str,
        class: String,
        attrs: Vec<(String, String)>,
        text: String,
        kids: Vec<Node>,
    },
    Page {
        title: String,
        theme: String,
        state: String,
        body: Box<Node>,
    },
}

pub(crate) fn dispatch(vm: &mut Vm, name: &str, args: Vec<Value>) -> Result<Value, Fault> {
    match name {
        "serve" => {
            serve(vm, int(&args, 0)?, text(&args, 1)?, None)?;
            Ok(Value::Null)
        }
        "serve_n" => {
            serve(vm, int(&args, 0)?, text(&args, 1)?, Some(int(&args, 2)?.max(0) as usize))?;
            Ok(Value::Null)
        }
        _ => {
            let node = node(vm, name, &args)?;
            Ok(Value::Host(vm.heap.alloc(SlotData::Host(HostObj::Ui(node)))))
        }
    }
}

fn node(vm: &Vm, name: &str, args: &[Value]) -> Result<Node, Fault> {
    let one = |i: usize| args.get(i).unwrap_or(&Value::Null);
    Ok(match name {
        "page" => Node::Page {
            title: text(args, 0)?.to_string(),
            theme: text(args, 1)?.to_string(),
            body: Box::new(el("div", "", flatten(vm, one(2))?)),
            state: {
                let state = text(args, 3)?;
                if state.is_empty() { "{}".into() } else { state.to_string() }
            },
        },
        "col" => el("div", "col", flatten(vm, one(0))?),
        "row" => el("div", "row", flatten(vm, one(0))?),
        "grid" => el_attr("div", "grid", vec![("style".into(), format!("--cols:{}", int(args, 0)?.clamp(1, 6)))], flatten(vm, one(1))?),
        "split" => el("div", "split", vec![el("div", "", flatten(vm, one(0))?), el("div", "", flatten(vm, one(1))?) ]),
        "sidebar" => el("div", "sidebar", vec![el("div", "rail", flatten(vm, one(0))?), el("div", "main", flatten(vm, one(1))?) ]),
        "h1" => txt("h1", "", text(args, 0)?),
        "h2" => txt("h2", "", text(args, 0)?),
        "h3" => txt("h3", "", text(args, 0)?),
        "p" => txt("p", "", text(args, 0)?),
        "muted" => txt("p", "muted", text(args, 0)?),
        "kicker" => txt("p", "kicker", text(args, 0)?),
        "display" => txt("p", "display", text(args, 0)?),
        "nav" => el("header", "nav", vec![
            txt("a", "brand", text(args, 0)?).with_attr("href", "#top"),
            el("nav", "links", flatten(vm, one(1))?),
        ]),
        "a" => txt("a", "link", text(args, 1)?).with_attr("href", text(args, 0)?),
        "button" => action_button("btn", text(args, 0)?, text(args, 1)?),
        "quiet" => action_button("btn quiet", text(args, 0)?, text(args, 1)?),
        "submit" => txt("button", "btn", text(args, 0)?).with_attr("type", "submit"),
        "card" => el("article", "card", vec![
            txt("h3", "", text(args, 0)?),
            txt("p", "", text(args, 1)?),
            txt("footer", "", text(args, 2)?),
        ]),
        "hero" => {
            let action = text(args, 3)?;
            let mut parts = vec![
                txt("p", "kicker", text(args, 0)?),
                txt("h1", "", text(args, 1)?),
                txt("p", "lede", text(args, 2)?),
            ];
            if !action.is_empty() {
                parts.push(action_button("btn", text(args, 4)?, action));
            }
            el("section", "hero", parts)
        }
        "price" => el("article", "price", vec![
            el("div", "", vec![txt("h3", "", text(args, 0)?), txt("p", "", text(args, 1)?)]),
            txt("b", "", text(args, 2)?),
        ]),
        "stat" => el("article", "stat", vec![
            txt("p", "kicker", text(args, 0)?),
            txt("p", "display", text(args, 1)?),
            txt("p", "muted", text(args, 2)?),
        ]),
        "badge" => txt("span", "badge", text(args, 0)?),
        "table" => table(text(args, 0)?, text(args, 1)?),
        "field" => el("label", "field", vec![
            txt("span", "", text(args, 0)?),
            txt("input", "", "").with_attr("name", text(args, 1)?).with_attr("value", text(args, 2)?).with_attr("type", "text"),
        ]),
        "anchor" => el_attr("section", "anchor", vec![("id".into(), safe_id(text(args, 0)?))], flatten(vm, one(1))?),
        "form" => el_attr("form", "form", vec![("data-act".into(), text(args, 0)?.to_string())], flatten(vm, one(1))?),
        "note" => {
            let text = text(args, 0)?;
            if text.is_empty() { Node::Empty } else { txt("p", "note", text) }
        }
        "device" => el("section", "device", flatten(vm, one(0))?),
        "screen" => txt("p", "screen", text(args, 0)?),
        "keys" => el("div", "keys", flatten(vm, one(0))?),
        "key" => {
            let kind = text(args, 2)?;
            let class = match kind {
                "op" => "key op",
                "quiet" => "key quiet",
                "eq" => "key eq",
                "wide" => "key wide",
                _ => "key",
            };
            action_button(class, text(args, 0)?, text(args, 1)?)
        }
        _ => return Err(Fault::Type(format!("unknown view {name}"))),
    })
}

fn el(tag: &'static str, class: &str, kids: Vec<Node>) -> Node {
    Node::El { tag, class: class.into(), attrs: Vec::new(), text: String::new(), kids: kids.into_iter().filter(|n| !matches!(n, Node::Empty)).collect() }
}

fn el_attr(tag: &'static str, class: &str, attrs: Vec<(String, String)>, kids: Vec<Node>) -> Node {
    Node::El { tag, class: class.into(), attrs, text: String::new(), kids: kids.into_iter().filter(|n| !matches!(n, Node::Empty)).collect() }
}

fn txt(tag: &'static str, class: &str, text: &str) -> Node {
    Node::El { tag, class: class.into(), attrs: Vec::new(), text: text.into(), kids: Vec::new() }
}

fn action_button(class: &str, label: &str, action: &str) -> Node {
    txt("button", class, label).with_attr("type", "button").with_attr("data-act", action)
}

impl Node {
    fn with_attr(mut self, key: &str, value: &str) -> Self {
        if let Node::El { attrs, .. } = &mut self {
            attrs.push((key.into(), value.into()));
        }
        self
    }
}

fn flatten(vm: &Vm, value: &Value) -> Result<Vec<Node>, Fault> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Arr(h) => {
            let slot = vm.heap.slot(*h)?;
            let SlotData::Arr { elems, .. } = &slot.data else {
                return Err(Fault::Type("array expected".into()));
            };
            let mut out = Vec::new();
            for item in elems {
                out.extend(flatten(vm, item)?);
            }
            Ok(out)
        }
        Value::Host(h) => {
            let slot = vm.heap.slot(*h)?;
            match &slot.data {
                SlotData::Host(HostObj::Ui(node)) => Ok(vec![node.clone()]),
                SlotData::Host(HostObj::List(xs)) => {
                    let mut out = Vec::new();
                    for item in xs {
                        out.extend(flatten(vm, item)?);
                    }
                    Ok(out)
                }
                _ => Err(Fault::Type("view expected".into())),
            }
        }
        _ => Err(Fault::Type("view expected".into())),
    }
}

fn table(headers: &str, rows: &str) -> Node {
    let headers: Vec<String> = serde_json::from_str::<Vec<String>>(headers).unwrap_or_default();
    let rows: Vec<Vec<String>> = serde_json::from_str(rows).unwrap_or_default();
    let mut head = Vec::new();
    for cell in &headers {
        head.push(txt("th", "", cell));
    }
    let mut body = Vec::new();
    for row in &rows {
        let cells = row.iter().map(|cell| txt("td", "", cell)).collect();
        body.push(el("tr", "", cells));
    }
    el("table", "table", vec![el("thead", "", vec![el("tr", "", head)]), el("tbody", "", body)])
}

fn text<'a>(args: &'a [Value], i: usize) -> Result<&'a str, Fault> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        _ => Err(Fault::Type("string expected".into())),
    }
}

fn int(args: &[Value], i: usize) -> Result<i64, Fault> {
    match args.get(i) {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err(Fault::Type("int expected".into())),
    }
}

fn serve(vm: &Vm, port: i64, handler: &str, limit: Option<usize>) -> Result<(), Fault> {
    if !(0..=65535).contains(&port) {
        return Err(Fault::Io(format!("bad port {port}")));
    }
    let listener = TcpListener::bind(("0.0.0.0", port as u16)).map_err(|e| Fault::Io(e.to_string()))?;
    let mut served = 0usize;
    for conn in listener.incoming() {
        let mut stream = conn.map_err(|e| Fault::Io(e.to_string()))?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let response = match read_http(&mut stream) {
            Ok((method, path, body)) => match answer(vm, handler, &method, &path, &body) {
                Ok((status, ctype, payload)) => http_response(status, ctype, &payload),
                Err(err) => http_response(500, "text/plain; charset=utf-8", &err),
            },
            Err(Fault::Io(err)) => http_response(400, "text/plain; charset=utf-8", &err),
            Err(_) => http_response(400, "text/plain; charset=utf-8", "bad request"),
        };
        let _ = stream.write_all(response.as_bytes());
        served += 1;
        if limit == Some(served) {
            break;
        }
    }
    Ok(())
}

fn answer(vm: &Vm, handler: &str, method: &str, path: &str, body: &str) -> Result<(u16, &'static str, String), String> {
    let route = path.split('?').next().unwrap_or("/");
    if method.eq_ignore_ascii_case("POST") && route == "/_ui" {
        let state = posted_state(body, route);
        let node = call_view(vm, handler, &state)?;
        let (title, theme, next, html) = page_parts(&node);
        let msg = serde_json::json!({ "html": html, "state": next, "theme": theme, "title": title });
        return Ok((200, "application/json", msg.to_string()));
    }
    let state = format!("{{\"path\":\"{route}\",\"action\":\"\",\"form\":{{}}}}");
    let node = call_view(vm, handler, &state)?;
    Ok((200, "text/html; charset=utf-8", document(&node)))
}

fn posted_state(body: &str, path: &str) -> String {
    let posted: Json = serde_json::from_str(body).unwrap_or(Json::Null);
    let action = posted.get("action").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let form = posted.get("form").cloned().unwrap_or_else(|| Json::Object(Map::new()));
    let mut doc: Json = posted
        .get("state")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| Json::Object(Map::new()));
    if let Json::Object(obj) = &mut doc {
        obj.insert("path".into(), Json::String(path.into()));
        obj.insert("action".into(), Json::String(action));
        obj.insert("form".into(), form);
        return doc.to_string();
    }
    serde_json::json!({ "path": path, "action": action, "form": form }).to_string()
}

fn call_view(vm: &Vm, handler: &str, state: &str) -> Result<Node, String> {
    let mut fresh = Vm::blank(vm.module.clone(), vm.lang);
    match fresh.call_named(handler, vec![Value::Str(state.into())]) {
        Ok(Value::Host(h)) => {
            let slot = fresh.heap.slot(h).map_err(|_| "view was freed".to_string())?;
            match &slot.data {
                SlotData::Host(HostObj::Ui(node)) => Ok(node.clone()),
                _ => Err("the view function must return a ui value".into()),
            }
        }
        Ok(_) => Err("the view function must return a ui value".into()),
        Err(Fault::Io(s)) => Err(s),
        Err(_) => Err("the view function failed".into()),
    }
}

fn page_parts(node: &Node) -> (String, String, String, String) {
    match node {
        Node::Page { title, theme, state, body } => (title.clone(), theme_name(theme).into(), state.clone(), render(body)),
        other => ("Terra".into(), "ledger".into(), "{}".into(), render(other)),
    }
}

fn theme_name(theme: &str) -> &str {
    match theme {
        "roast" | "instrument" | "ledger" => theme,
        _ => "ledger",
    }
}

fn document(node: &Node) -> String {
    let (title, theme, state, html) = page_parts(node);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=Newsreader:ital,opsz,wght@0,6..72,450;0,6..72,550;1,6..72,450&display=swap" rel="stylesheet">
<style>{CSS}</style>
</head>
<body data-theme="{theme}">
<div id="app" data-state="{state}">{html}</div>
<script>{SCRIPT}</script>
</body>
</html>"#,
        title = esc(&title),
        theme = esc(&theme),
        state = esc(&state),
        html = html,
    )
}

fn safe_id(text: &str) -> String {
    let ok = !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok { text.into() } else { "section".into() }
}

fn http_response(status: u16, ctype: &str, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        500 => "Error",
        _ => "OK",
    };
    format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn render(node: &Node) -> String {
    match node {
        Node::Empty => String::new(),
        Node::Page { body, .. } => render(body),
        Node::El { tag, class, attrs, text, kids } => {
            let mut html = format!("<{tag}");
            if !class.is_empty() {
                html.push_str(&format!(" class=\"{}\"", esc(class)));
            }
            for (key, value) in attrs {
                html.push_str(&format!(" {}=\"{}\"", esc(key), esc(value)));
            }
            html.push('>');
            html.push_str(&esc(text));
            for kid in kids {
                html.push_str(&render(kid));
            }
            if !matches!(*tag, "input" | "img") {
                html.push_str(&format!("</{tag}>"));
            }
            html
        }
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const SCRIPT: &str = r#"
(function () {
  var root = document.getElementById("app");
  function send(action, form) {
    fetch("/_ui", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ action: action, form: form || {}, state: root.getAttribute("data-state") || "{}" })
    }).then(function (r) { return r.json(); }).then(function (msg) {
      root.innerHTML = msg.html;
      root.setAttribute("data-state", msg.state);
      document.body.setAttribute("data-theme", msg.theme);
      document.title = msg.title;
    });
  }
  document.addEventListener("click", function (e) {
    var t = e.target.closest ? e.target.closest("[data-act]") : null;
    if (!t || t.tagName === "FORM") return;
    e.preventDefault();
    send(t.getAttribute("data-act"), {});
  });
  document.addEventListener("submit", function (e) {
    var form = e.target;
    if (!form || form.tagName !== "FORM") return;
    e.preventDefault();
    var data = {};
    var fields = form.querySelectorAll("input, textarea");
    for (var i = 0; i < fields.length; i++) data[fields[i].name] = fields[i].value;
    send(form.getAttribute("data-act") || "submit", data);
  });
})();
"#;

const CSS: &str = r#"
:root {
  color-scheme: dark;
  --bg: #101614;
  --bg-2: #17211e;
  --ink: #e7f0ea;
  --muted: #9aafa6;
  --line: #2a3b36;
  --accent: #d7b48a;
  --accent-ink: #1a140e;
  --card: #1b2622;
  --danger: #e0b4a8;
  --shadow: 0 24px 60px rgba(0, 0, 0, .28);
  --serif: Newsreader, Georgia, serif;
  --sans: Inter, "Segoe UI", sans-serif;
}
[data-theme="roast"] {
  --bg: #14110e;
  --bg-2: #1d1914;
  --ink: #f4ecdf;
  --muted: #b5a894;
  --line: #3a3128;
  --accent: #d7b48a;
  --accent-ink: #1a140e;
  --card: #221c17;
}
[data-theme="instrument"] {
  --bg: #0e1418;
  --bg-2: #151d23;
  --ink: #e8eef2;
  --muted: #93a3ae;
  --line: #2a3942;
  --accent: #f2c14e;
  --accent-ink: #1c1608;
  --card: #182229;
  --serif: Inter, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
  margin: 0;
  min-height: 100vh;
  background:
    radial-gradient(900px 420px at 10% -10%, color-mix(in srgb, var(--accent) 16%, transparent), transparent 60%),
    var(--bg);
  color: var(--ink);
  font-family: var(--sans);
  font-size: 17px;
  line-height: 1.5;
}
#app { min-height: 100vh; }
.col { display: flex; flex-direction: column; gap: 1.25rem; }
.row { display: flex; flex-wrap: wrap; gap: 1rem; align-items: stretch; }
.grid { display: grid; grid-template-columns: repeat(var(--cols, 1), minmax(0, 1fr)); gap: 1rem; }
.split { display: grid; grid-template-columns: 1.1fr .9fr; gap: 2.5rem; align-items: end; max-width: 72rem; margin: 0 auto; padding: 1rem 1.5rem 3rem; }
.sidebar { display: grid; grid-template-columns: 16rem minmax(0, 1fr); min-height: 100vh; }
.rail { padding: 2rem 1.4rem; background: var(--bg-2); border-right: 1px solid var(--line); }
.main { padding: 2rem 1.6rem 3rem; display: flex; flex-direction: column; gap: 1.25rem; }
h1, h2, h3 { font-family: var(--serif); font-weight: 550; letter-spacing: -0.03em; margin: 0; line-height: 1.05; }
h1 { font-size: clamp(3.2rem, 8vw, 6.4rem); }
h2 { font-size: clamp(2rem, 4vw, 3rem); }
h3 { font-size: 1.35rem; }
p { margin: 0; }
.kicker { color: var(--accent); font-family: var(--sans); font-size: .78rem; font-weight: 600; letter-spacing: .14em; text-transform: uppercase; }
.muted, .lede { color: var(--muted); max-width: 38rem; }
.display { font-variant-numeric: tabular-nums; font-size: clamp(2.4rem, 5vw, 3.4rem); font-weight: 560; letter-spacing: -0.04em; }
.nav { display: flex; justify-content: space-between; align-items: center; gap: 1rem; padding: 1.1rem 1.5rem; position: sticky; top: 0; background: color-mix(in srgb, var(--bg) 84%, transparent); backdrop-filter: blur(10px); border-bottom: 1px solid transparent; }
.brand { color: inherit; text-decoration: none; font-family: var(--serif); font-size: 1.35rem; font-style: italic; }
.links { display: flex; gap: 1.1rem; }
.link { color: var(--muted); text-decoration: none; }
.link:hover { color: var(--ink); }
.hero { max-width: 72rem; margin: 0 auto; padding: 4.5rem 1.5rem 2rem; display: flex; flex-direction: column; gap: .9rem; }
.hero h1 { max-width: 11ch; }
.card, .stat, .price, .note { background: var(--card); border: 1px solid var(--line); border-radius: 18px; }
.card, .stat { padding: 1.15rem 1.2rem 1.2rem; display: flex; flex-direction: column; gap: .55rem; }
.card footer { color: var(--accent); font-variant-numeric: tabular-nums; }
.price { padding: 1rem 1.15rem; display: flex; justify-content: space-between; gap: 1rem; align-items: end; }
.price b { font-weight: 560; font-variant-numeric: tabular-nums; font-size: 1.15rem; }
.stat { min-width: 12rem; flex: 1; }
.badge { display: inline-flex; align-items: center; border: 1px solid var(--line); border-radius: 999px; padding: .15rem .6rem; color: var(--accent); font-size: .85rem; }
.note { padding: .9rem 1rem; color: var(--accent-ink); background: var(--accent); border: 0; }
.table { width: 100%; border-collapse: collapse; }
.table th, .table td { text-align: left; padding: .75rem .4rem; border-bottom: 1px solid var(--line); font-weight: 500; }
.table th { color: var(--muted); font-size: .82rem; font-weight: 600; }
.form { display: grid; grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr)); gap: .8rem; align-items: end; }
.field { display: flex; flex-direction: column; gap: .35rem; color: var(--muted); font-size: .85rem; }
.field input { font: inherit; color: var(--ink); background: transparent; border: 0; border-bottom: 1px solid var(--line); padding: .55rem 0; outline: none; }
.field input:focus { border-bottom-color: var(--accent); }
.btn, .key { font: inherit; cursor: pointer; border: 0; border-radius: 999px; padding: .75rem 1.1rem; background: var(--accent); color: var(--accent-ink); transition: transform .16s ease, background .16s ease; }
.btn.quiet, .key.quiet { background: transparent; color: var(--ink); border: 1px solid var(--line); }
.btn:hover, .key:hover { transform: translateY(-1px); }
.btn:active, .key:active { transform: translateY(0) scale(.98); }
.device { width: min(24rem, calc(100% - 2rem)); margin: 8vh auto; background: var(--card); border: 1px solid var(--line); border-radius: 28px; padding: 1.1rem; box-shadow: var(--shadow); display: flex; flex-direction: column; gap: .8rem; }
.screen { min-height: 4.6rem; margin: 0; padding: .4rem .3rem 0; text-align: right; font-size: 3rem; font-weight: 520; letter-spacing: -0.04em; font-variant-numeric: tabular-nums; }
.keys { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: .55rem; }
.key { border-radius: 16px; min-height: 3.4rem; background: var(--bg-2); color: var(--ink); }
.key.op { color: var(--accent); }
.key.eq { background: var(--accent); color: var(--accent-ink); }
.key.wide { grid-column: span 2; }
.anchor { max-width: 72rem; margin: 0 auto; padding: 1.2rem 1.5rem 2.6rem; display: flex; flex-direction: column; gap: 1rem; }
.hero::after { content: ""; width: 4.5rem; height: 1px; background: var(--accent); }
.hero, .stat, .card { animation: rise .55s ease both; }
.stat:nth-child(2) { animation-delay: .08s; }
@keyframes rise { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: none; } }
:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
@media (max-width: 800px) {
  .grid, .split, .sidebar, .form { grid-template-columns: 1fr; }
  .sidebar { display: block; }
  .rail { border-right: 0; border-bottom: 1px solid var(--line); }
  .hero { padding-top: 2.5rem; }
  h1 { font-size: 3.1rem; }
}
@media (prefers-reduced-motion: reduce) {
  * { animation: none !important; transition: none !important; scroll-behavior: auto !important; }
}
"#;
