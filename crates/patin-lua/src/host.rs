//! The Lua host: what the VM is given, how the chunk is run, and how what it
//! assigned is carried back out.
//!
//! Patin embeds [luna](https://github.com/onix-os/luna), a stackless Lua VM.
//! Stackless matters here for one concrete reason: control returns to this
//! loop between slices, so a config with an accidental `while true do end` is
//! stopped by a fuel budget instead of hanging a lock screen forever. A
//! conventional interpreter would be inside its own `for` loop with no way to
//! ask it to stop.
//!
//! The VM is started with `Lua::core()` — base, string, table, math,
//! coroutine, utf8 — and deliberately not `Lua::full()`, which would add
//! `io`, `os`, `package`, and `debug`. A config still has to be able to adapt
//! to the machine it is running on, so the few probes it genuinely needs are
//! handed in as host functions instead: they are bounded, they can be given
//! useful errors, and none of them can block a frame the way an arbitrary
//! `io.open` on a slow filesystem would.

use std::collections::BTreeMap;
use std::path::PathBuf;

use luna::{
    Callback, CallbackReturn, Closure, Executor, Fuel, IntoValue, Lua, Table as LuaTable,
    Value as LuaValue,
};

use crate::value::{Table, Value};

/// Namespaces created before the chunk runs, so a config can write
/// `patin.theme.accent = ...` without first writing `patin.theme = {}`.
pub(crate) const NAMESPACES: &[&str] = &[
    "theme", "bar", "lock", "login", "session", "launcher", "osk", "status", "actions",
];

/// Host functions living on the module table. Harvesting skips them: they are
/// Patin's, not the config's, and reporting them back as unknown settings
/// would be noise.
const HOST_FUNCTIONS: &[&str] = &["env", "exists", "which", "log"];

/// How much work `init.lua` may do. A config computes a handful of colours and
/// a few conditionals; the ceiling exists only to bound a mistake, so it is
/// set far above any honest file and far below "hangs the greeter".
const FUEL_BUDGET: i64 = 8_000_000;
const FUEL_PER_SLICE: i32 = 4096;
const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
/// Tables nested deeper than this are dropped rather than followed, which is
/// also what stops `patin.self = patin` from recursing forever.
const MAX_DEPTH: usize = 8;

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    Run {
        path: PathBuf,
        message: String,
    },
    Budget {
        path: PathBuf,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            // luna's parse errors carry a line but not the chunk name, so the
            // path is put back in front of it here: a config error that does
            // not say which file it is in is most of the way to useless once
            // plugins can also be loaded.
            ConfigError::Parse { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            ConfigError::Run { path, message } => write!(f, "{}: {message}", path.display()),
            ConfigError::Budget { path } => write!(
                f,
                "{}: stopped after {FUEL_BUDGET} VM steps — a loop that never ends?",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Run one config chunk and return everything it assigned onto `patin`.
pub(crate) fn evaluate(path: &std::path::Path, source: &[u8]) -> Result<Table, ConfigError> {
    let mut lua = Lua::core();
    lua.set_memory_limit(Some(MEMORY_LIMIT));

    lua.enter(|ctx| {
        let module = LuaTable::new(&ctx);
        for name in NAMESPACES {
            let namespace = LuaTable::new(&ctx);
            // `set` only fails on a nil or NaN key; these are &'static str.
            let _ = module.set(ctx, *name, namespace);
        }
        install_host_functions(ctx, module);
        ctx.set_global("patin", module);

        // `require` under `Lua::core()`, which has no `package` library. It
        // serves the module table and nothing else, so the familiar first
        // line of a config works and a mistyped module name gets a real
        // message rather than a nil index three lines later.
        let require = Callback::from_fn(&ctx, |ctx, _exec, mut stack| {
            let name: luna::String = stack.consume(ctx)?;
            if name.as_bytes() == b"patin" {
                stack.push_back(ctx.get_global_value("patin"));
                Ok(CallbackReturn::Return)
            } else {
                let text = String::from_utf8_lossy(name.as_bytes()).into_owned();
                Err(
                    format!("no module '{text}'; Patin's config provides only 'patin'")
                        .into_value(ctx)
                        .into(),
                )
            }
        });
        ctx.set_global("require", require);
    });

    let name = path.display().to_string();
    let executor = lua
        .try_enter(|ctx| {
            let closure = Closure::load(ctx, Some(name.as_str()), source)?;
            Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
        })
        .map_err(|error| ConfigError::Parse {
            path: path.to_path_buf(),
            message: strip_prefix(&error.to_string()),
        })?;

    // Stepped by hand rather than through `Lua::finish`, which runs to
    // completion however long that takes.
    let mut spent: i64 = 0;
    loop {
        let mut fuel = Fuel::with(FUEL_PER_SLICE);
        let finished = lua.enter(|ctx| ctx.fetch(&executor).step(ctx, &mut fuel));
        spent += i64::from(FUEL_PER_SLICE - fuel.remaining());
        match finished {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) => {
                return Err(ConfigError::Run {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                });
            }
        }
        if spent > FUEL_BUDGET {
            lua.enter(|ctx| ctx.fetch(&executor).stop(&ctx));
            return Err(ConfigError::Budget {
                path: path.to_path_buf(),
            });
        }
    }

    lua.execute::<()>(&executor)
        .map_err(|error| ConfigError::Run {
            path: path.to_path_buf(),
            message: strip_prefix(&error.to_string()),
        })?;

    Ok(lua.enter(|ctx| {
        let module: LuaTable = match ctx.get_global("patin") {
            Ok(table) => table,
            // Only reachable if the config replaced `patin` with a non-table,
            // which is a config that configured nothing.
            Err(_) => return Table::default(),
        };
        harvest(ctx, module, 0)
    }))
}

fn install_host_functions<'gc>(ctx: luna::Context<'gc>, module: LuaTable<'gc>) {
    let env = Callback::from_fn(&ctx, |ctx, _exec, mut stack| {
        let name: luna::String = stack.consume(ctx)?;
        let value = std::str::from_utf8(name.as_bytes())
            .ok()
            .and_then(|name| std::env::var(name).ok());
        match value {
            Some(value) => stack.push_back(LuaValue::String(ctx.intern(value.as_bytes()))),
            None => stack.push_back(LuaValue::Nil),
        }
        Ok(CallbackReturn::Return)
    });

    let exists = Callback::from_fn(&ctx, |ctx, _exec, mut stack| {
        let path: luna::String = stack.consume(ctx)?;
        let found = std::str::from_utf8(path.as_bytes())
            .map(|path| std::path::Path::new(path).exists())
            .unwrap_or(false);
        stack.push_back(LuaValue::Boolean(found));
        Ok(CallbackReturn::Return)
    });

    let which = Callback::from_fn(&ctx, |ctx, _exec, mut stack| {
        let name: luna::String = stack.consume(ctx)?;
        let found = std::str::from_utf8(name.as_bytes())
            .ok()
            .and_then(which_path);
        match found {
            Some(path) => stack.push_back(LuaValue::String(ctx.intern(path.as_bytes()))),
            None => stack.push_back(LuaValue::Nil),
        }
        Ok(CallbackReturn::Return)
    });

    let log = Callback::from_fn(&ctx, |ctx, _exec, mut stack| {
        let message: luna::String = stack.consume(ctx)?;
        eprintln!(
            "patin config: {}",
            String::from_utf8_lossy(message.as_bytes())
        );
        Ok(CallbackReturn::Return)
    });

    let _ = module.set(ctx, "env", env);
    let _ = module.set(ctx, "exists", exists);
    let _ = module.set(ctx, "which", which);
    let _ = module.set(ctx, "log", log);
}

/// `$PATH` lookup for an executable file, so a config can ask whether the
/// program it wants to bind to a menu row is actually installed.
fn which_path(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;

    if name.contains('/') {
        let path = std::path::Path::new(name);
        return path.is_file().then(|| name.to_string());
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH")?) {
        let candidate = directory.join(name);
        let Ok(metadata) = std::fs::metadata(&candidate) else {
            continue;
        };
        if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn harvest<'gc>(ctx: luna::Context<'gc>, table: LuaTable<'gc>, depth: usize) -> Table {
    let mut array: BTreeMap<i64, Value> = BTreeMap::new();
    let mut map = BTreeMap::new();
    for (key, value) in table.iter(ctx) {
        match key {
            LuaValue::String(name) => {
                let name = String::from_utf8_lossy(name.as_bytes()).into_owned();
                if depth == 0 && HOST_FUNCTIONS.contains(&name.as_str()) {
                    continue;
                }
                if let Some(value) = convert(ctx, value, depth) {
                    map.insert(name, value);
                }
            }
            LuaValue::Integer(index) if index >= 1 => {
                if let Some(value) = convert(ctx, value, depth) {
                    array.insert(index, value);
                }
            }
            _ => {}
        }
    }
    // Only a contiguous 1..n run is an array; a hole means the rest was meant
    // as something else and guessing at it would invent entries the config
    // never wrote.
    let mut ordered = Vec::new();
    for (expected, (index, value)) in (1i64..).zip(array) {
        if index != expected {
            break;
        }
        ordered.push(value);
    }
    Table {
        array: ordered,
        map,
    }
}

fn convert<'gc>(ctx: luna::Context<'gc>, value: LuaValue<'gc>, depth: usize) -> Option<Value> {
    Some(match value {
        LuaValue::Nil => return None,
        LuaValue::Boolean(value) => Value::Boolean(value),
        LuaValue::Integer(value) => Value::Integer(value),
        LuaValue::Number(value) => Value::Number(value),
        LuaValue::String(value) => {
            Value::String(String::from_utf8_lossy(value.as_bytes()).into_owned())
        }
        LuaValue::Table(table) => {
            if depth + 1 > MAX_DEPTH {
                return None;
            }
            Value::Table(harvest(ctx, table, depth + 1))
        }
        LuaValue::Function(_) => Value::Function,
        LuaValue::Thread(_) | LuaValue::UserData(_) => return None,
    })
}

/// luna prefixes its messages with `lua error:` / `runtime error:`, which
/// reads oddly once Patin has already said which file failed.
fn strip_prefix(message: &str) -> String {
    message
        .strip_prefix("runtime error: ")
        .or_else(|| message.strip_prefix("lua error: "))
        .unwrap_or(message)
        .to_string()
}
