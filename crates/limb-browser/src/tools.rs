//! What the browser limb offers: its tools, the instructions for them, and
//! the JavaScript a `browser` script runs on.

use harness_core::environment::{Audience, Tool};
use serde_json::json;

pub const TAB: &str = "tab";
pub const BROWSER: &str = "browser";
pub const PAGE: &str = "page";
pub const SCREENSHOT: &str = "screenshot";

/// Whether a call cut off part way may simply run again: only what reads.
pub fn rerun(tool: &str) -> bool {
    matches!(tool, PAGE | SCREENSHOT)
}

pub fn tools() -> Vec<Tool> {
    vec![
        tool(
            TAB,
            "Run only through `background`: a tab of your own, open for as long as that background call runs, \
whose `name` is the tab's name in your scripts: `background` with `{\"name\": \"bank\", \"tool\": \"tab\", \"input\": {\"url\": \"https://…\"}}`. \
Stopping the background call closes the tab; `pass` hands the tab, still open, to whoever started you.",
            json!({ "url": { "type": "string", "description": "Where it opens; about:blank if absent." } }),
            &[],
        ),
        tool(
            BROWSER,
            "Run a script that drives the browser: the body of an async function in a JavaScript sandbox, \
whose `return` value (JSON) and `console.log` lines come back. It reaches the browser only through `tabs()` and \
`tab(name)`; see the browser's instructions. Do a whole step in one script.",
            json!({ "script": { "type": "string", "description": "The function body." } }),
            &["script"],
        ),
        tool(
            PAGE,
            "Run JavaScript in a tab's page and get back what it returns, as JSON: the body of an async function, \
with the page's `document` and `window`. How to read a page: its text, its form's fields, a table as rows.",
            json!({
                "tab": { "type": "string", "description": "The tab's name, as `tabs()` lists it." },
                "script": { "type": "string", "description": "The function body; `return` what you want back." }
            }),
            &["tab", "script"],
        ),
        tool(
            SCREENSHOT,
            "A picture of what a tab shows now, its viewport at most 1280 pixels wide. Costly: use it to get \
the gestalt of a page you haven't seen, or when scripts cannot tell you what is happening.",
            json!({ "tab": { "type": "string", "description": "The tab's name." } }),
            &["tab"],
        ),
    ]
}

fn tool(name: &str, description: &str, properties: serde_json::Value, required: &[&str]) -> Tool {
    let definition = json!({
        "name": name,
        "description": description,
        "input_schema": { "type": "object", "properties": properties, "required": required },
    });
    Tool {
        name: name.into(),
        definition: definition.to_string(),
        audience: Audience::Both,
    }
}

/// How to use the browser, rendered into each root that holds it.
pub fn instructions(where_: &str) -> String {
    format!(
        "## The browser\n\n\
The harness's browser: {where_}. It is shared: other agents' tabs are in it too, and you see only yours and Max's.\n\n\
**Tabs.** Open a tab of your own by running the `tab` tool with `background`; its background name is the tab's \
name. It stays open while that call runs: `stop` it to close the tab, or `pass` it to hand the tab on, open. A tab \
Max opened himself is his, named by its id in `tabs()`; work in it only when your task says to, and never close it.\n\n\
**Scripts.** `browser` runs a script in a sandbox with these, and nothing else (no `fetch`, no timers):\n\
- `tabs()`: `[{{tab, url, title, whose}}]`, the tabs you can use.\n\
- `tab(name)`: a tab, with\n\
  - `.goto(url)`: navigate and wait for the page to load.\n\
  - `.eval(fn, ...args)`: run `fn` in the page (a function or an expression string) and return its JSON result.\n\
  - `.click(selector)`: a real mouse click at the element's centre, scrolled into view.\n\
  - `.fill(selector, text)`: replace a field's value as if typed; a `<select>` is set by value or label.\n\
  - `.press(key)`: a key such as `Enter`, `Tab`, `Escape`, `ArrowDown`, `Backspace`, or one character.\n\
  - `.waitFor(selectorOrFn, ms = 10000)`: until the selector matches or the function returns truthy.\n\
  - `.url()`, `.title()`, `.text(selector?)`: the URL, the title, an element's (or the page's) text.\n\
Each call returns when it is done, so `await` is optional. A script may run for 2 minutes.\n"
    )
}

/// The JavaScript a `browser` script runs after, over the host's
/// functions (`__`-named).
pub const PRELUDE: &str = r#"
function tabs() { return __tabs(); }
function tab(name) {
  const t = __find(String(name));
  const expr = (f, args) => typeof f === 'function' ? `(${f})(...${JSON.stringify(args)})` : String(f);
  return {
    name: t,
    goto: (url) => __goto(t, String(url)),
    eval: (f, ...args) => __eval(t, expr(f, args)),
    click: (selector) => __click(t, String(selector)),
    fill: (selector, text) => __fill(t, String(selector), String(text)),
    press: (key) => __press(t, String(key)),
    waitFor: (what, ms) => __wait(t, typeof what === 'function' ? expr(what, []) :
      `document.querySelector(${JSON.stringify(String(what))})`, ms ?? 10000),
    url: () => __eval(t, 'location.href'),
    title: () => __eval(t, 'document.title'),
    text: (selector) => __eval(t, selector ?
      `document.querySelector(${JSON.stringify(String(selector))})?.innerText ?? null` : 'document.body.innerText'),
  };
}
"#;

/// The page-side half of `fill`: focus the field and select what it holds,
/// or set a `<select>` outright. Returns `"select"` when it set it.
pub fn fill_script(selector: &str, text: &str) -> String {
    format!(
        r#"(() => {{
  const el = document.querySelector({selector});
  if (!el) throw new Error('no element matches ' + {selector});
  el.scrollIntoView({{block: 'center'}});
  if (el.tagName === 'SELECT') {{
    const o = [...el.options].find(o => o.value === {text} || o.label.trim() === {text});
    if (!o) throw new Error('no option ' + {text} + ' in ' + {selector});
    el.value = o.value;
    el.dispatchEvent(new Event('input', {{bubbles: true}}));
    el.dispatchEvent(new Event('change', {{bubbles: true}}));
    return 'select';
  }}
  el.focus();
  if (typeof el.select === 'function') el.select();
  else document.execCommand('selectAll');
  return 'typed';
}})()"#,
        selector = json!(selector),
        text = json!(text),
    )
}

/// The page-side half of `click`: the centre of the element, in the
/// viewport, after scrolling it into view.
pub fn centre_script(selector: &str) -> String {
    format!(
        r#"(() => {{
  const el = document.querySelector({selector});
  if (!el) throw new Error('no element matches ' + {selector});
  el.scrollIntoView({{block: 'center', inline: 'center'}});
  const r = el.getBoundingClientRect();
  if (r.width === 0 && r.height === 0) throw new Error({selector} + ' is not visible');
  return {{x: r.x + r.width / 2, y: r.y + r.height / 2}};
}})()"#,
        selector = json!(selector),
    )
}

/// A key's DevTools description: its `key`, `code`, key code, and the text
/// it types.
pub fn key(name: &str) -> Option<(String, String, u32, Option<String>)> {
    let named = |key: &str, code: u32, text: Option<&str>| {
        Some((key.into(), key.into(), code, text.map(str::to_string)))
    };
    match name {
        "Enter" => named("Enter", 13, Some("\r")),
        "Tab" => named("Tab", 9, None),
        "Escape" => named("Escape", 27, None),
        "Backspace" => named("Backspace", 8, None),
        "Delete" => named("Delete", 46, None),
        "ArrowUp" => named("ArrowUp", 38, None),
        "ArrowDown" => named("ArrowDown", 40, None),
        "ArrowLeft" => named("ArrowLeft", 37, None),
        "ArrowRight" => named("ArrowRight", 39, None),
        "Home" => named("Home", 36, None),
        "End" => named("End", 35, None),
        "PageUp" => named("PageUp", 33, None),
        "PageDown" => named("PageDown", 34, None),
        " " | "Space" => Some((" ".into(), "Space".into(), 32, Some(" ".into()))),
        one if one.chars().count() == 1 => {
            let c = one.chars().next()?;
            let code = c.to_ascii_uppercase() as u32;
            Some((one.into(), String::new(), code, Some(one.into())))
        }
        _ => None,
    }
}
