//! Scripts an agent writes, run in QuickJS.
//!
//! A script reaches nothing but the functions its host hands it: no files,
//! no network, no processes, no clock to sleep on. So a limb that runs an
//! agent's JavaScript (the browser limb's `browser` tool, a later sandbox)
//! decides exactly what it can do by what it hands over, which is the
//! harness's access control: an environment either holds the limb or does
//! not.
//!
//! A script is the body of an async function: it may `await` (the host's
//! functions return at once, so awaiting them is harmless and habit-proof)
//! and `return` what it wants back. What it returns, and what it printed
//! with `console.log`, are JSON.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use rquickjs::function::Rest;
use rquickjs::{CatchResultExt, Context, Ctx, Exception, Function, Promise, Runtime, Value};
use serde_json::Value as Json;

/// A function the host hands a script: JSON arguments in, JSON or a
/// message (thrown as an `Error`) out.
pub type Host = Rc<dyn Fn(&[Json]) -> Result<Json, String>>;

/// How a script ended.
pub struct Ran {
    /// What it returned (`null` for nothing), or why it failed: its
    /// exception's message and stack.
    pub value: Result<Json, String>,
    /// Each `console.log`, its arguments joined by spaces.
    pub printed: Vec<String>,
}

/// Run `body` with `functions` as globals and `prelude` (the host's own
/// JavaScript, which may wrap them) before it. It is stopped when `stop`
/// says so or at `deadline`.
pub fn run(
    prelude: &str,
    body: &str,
    functions: &[(&str, Host)],
    deadline: Instant,
    stop: impl Fn() -> bool + 'static,
) -> Ran {
    let printed = Rc::new(RefCell::new(Vec::new()));
    let value = (|| {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_interrupt_handler(Some(Box::new(move || stop() || Instant::now() >= deadline)));
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let value = context.with(|ctx| eval(&ctx, prelude, body, functions, printed.clone()));
        if value.as_ref().is_err_and(|e| e.contains("interrupted")) {
            return Err(if Instant::now() >= deadline {
                "the script ran out of time".into()
            } else {
                "the script was stopped".into()
            });
        }
        value
    })();
    let printed = printed.take();
    Ran { value, printed }
}

fn eval<'js>(
    ctx: &Ctx<'js>,
    prelude: &str,
    body: &str,
    functions: &[(&str, Host)],
    printed: Rc<RefCell<Vec<String>>>,
) -> Result<Json, String> {
    let globals = ctx.globals();
    for (name, f) in functions {
        let f = f.clone();
        let js = Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            let args = args
                .0
                .into_iter()
                .map(|a| to_json(&ctx, a))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| Exception::throw_message(&ctx, &e))?;
            match f(&args) {
                Ok(value) => from_json(&ctx, &value),
                Err(message) => Err(Exception::throw_message(&ctx, &message)),
            }
        })
        .map_err(|e| e.to_string())?;
        globals.set(*name, js).map_err(|e| e.to_string())?;
    }
    let log = Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
        let line = args
            .0
            .into_iter()
            .map(|a| match a.as_string() {
                Some(s) => s.to_string().unwrap_or_default(),
                None => to_json(&ctx, a).map(|j| j.to_string()).unwrap_or_default(),
            })
            .collect::<Vec<_>>()
            .join(" ");
        printed.borrow_mut().push(line);
    })
    .map_err(|e| e.to_string())?;
    let console = rquickjs::Object::new(ctx.clone()).map_err(|e| e.to_string())?;
    console.set("log", log).map_err(|e| e.to_string())?;
    globals.set("console", console).map_err(|e| e.to_string())?;

    let caught = |e: rquickjs::CaughtError| e.to_string();
    ctx.eval::<(), _>(prelude).catch(ctx).map_err(caught)?;
    let source = format!("(async () => {{\n{body}\n}})()");
    let promise: Promise = ctx.eval(source).catch(ctx).map_err(caught)?;
    let value: Value = promise.finish().catch(ctx).map_err(caught)?;
    to_json(ctx, value)
}

fn to_json<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Json, String> {
    match ctx.json_stringify(value).map_err(|e| e.to_string())? {
        Some(text) => {
            let text = text.to_string().map_err(|e| e.to_string())?;
            serde_json::from_str(&text).map_err(|e| e.to_string())
        }
        None => Ok(Json::Null),
    }
}

fn from_json<'js>(ctx: &Ctx<'js>, value: &Json) -> rquickjs::Result<Value<'js>> {
    ctx.json_parse(value.to_string())
}
