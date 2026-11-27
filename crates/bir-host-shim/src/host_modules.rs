//! A UI-free copy of `gpui_shell::host_modules` (gpui-component rev
//! a0230db, `crates/shell/src/host_modules.rs`): the plain-data host boundary
//! and the `HostModule` registry, without element components and without the
//! engine-side `HostModules` registry.
//!
//! `bir-app-api` imports this crate under the name `gpui_shell`, so the host
//! functions moved out of `bir-shell` compile with their existing paths. The
//! definitions, messages and behavior match the original; the one addition is
//! [`HostModule::functions`].

use std::{collections::BTreeMap, fmt, fmt::Write as _, future::Future, pin::Pin, rc::Rc};

/// Whether a registered function answers directly or with a promise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
    Sync,
    Async,
}

/// A value crossing the host boundary, in either direction.
///
/// The six cases are the intersection of what a script engine and JSON can both
/// carry, which is what keeps one registry usable from any engine behind the
/// seam rather than from QuickJS alone.
#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<HostValue>),
    /// Insertion-ordered, because an object is frequently a record the script
    /// renders in order, and a map would decide that order for it.
    Object(Vec<(String, HostValue)>),
}

impl HostValue {
    pub fn is_null(&self) -> bool {
        matches!(self, HostValue::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            HostValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            HostValue::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            HostValue::Str(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[HostValue]> {
        match self {
            HostValue::Array(values) => Some(values),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, HostValue)]> {
        match self {
            HostValue::Object(fields) => Some(fields),
            _ => None,
        }
    }

    /// The value of one object field, or `None` for a non-object.
    pub fn get(&self, key: &str) -> Option<&HostValue> {
        self.as_object()?
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// The type name used in error messages.
    pub fn describe(&self) -> &'static str {
        match self {
            HostValue::Null => "null",
            HostValue::Bool(_) => "a boolean",
            HostValue::Number(_) => "a number",
            HostValue::Str(_) => "a string",
            HostValue::Array(_) => "an array",
            HostValue::Object(_) => "an object",
        }
    }
}

impl From<bool> for HostValue {
    fn from(value: bool) -> Self {
        HostValue::Bool(value)
    }
}

macro_rules! from_number {
    ($($type:ty),* $(,)?) => {
        $(
            impl From<$type> for HostValue {
                fn from(value: $type) -> Self {
                    HostValue::Number(value as f64)
                }
            }
        )*
    };
}

from_number!(f32, f64, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl From<String> for HostValue {
    fn from(value: String) -> Self {
        HostValue::Str(value)
    }
}

impl From<&str> for HostValue {
    fn from(value: &str) -> Self {
        HostValue::Str(value.to_owned())
    }
}

impl<T: Into<HostValue>> From<Vec<T>> for HostValue {
    fn from(values: Vec<T>) -> Self {
        HostValue::Array(values.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<HostValue>> From<Option<T>> for HostValue {
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(HostValue::Null)
    }
}

/// Builds a [`HostValue::Object`] one field at a time.
///
/// A record is the common return shape — a row, a settings snapshot, a
/// progress report — and building it with a builder keeps the field order the
/// host wrote it in.
#[derive(Clone, Debug, Default)]
pub struct HostObject {
    fields: Vec<(String, HostValue)>,
}

impl HostObject {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a field. A repeated name replaces the earlier value in place, so a
    /// caller cannot accidentally emit the same key twice.
    pub fn field(mut self, name: impl Into<String>, value: impl Into<HostValue>) -> Self {
        let name = name.into();
        let value = value.into();
        match self.fields.iter_mut().find(|(key, _)| *key == name) {
            Some(existing) => existing.1 = value,
            None => self.fields.push((name, value)),
        }
        self
    }
}

impl From<HostObject> for HostValue {
    fn from(object: HostObject) -> Self {
        HostValue::Object(object.fields)
    }
}

/// The positional arguments of one host call.
///
/// The typed readers exist so that a wrong argument reports which position was
/// wrong and what arrived there, rather than the host writing that sentence
/// once per function.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostArguments {
    values: Vec<HostValue>,
}

impl HostArguments {
    pub fn new(values: impl IntoIterator<Item = HostValue>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&HostValue> {
        self.values.get(index)
    }

    /// The argument at `index`, or an error naming the position.
    pub fn value(&self, index: usize) -> Result<&HostValue, HostError> {
        self.values.get(index).ok_or_else(|| {
            HostError::new(format!(
                "argument {} is missing; {} were passed",
                index + 1,
                self.values.len()
            ))
        })
    }

    pub fn string(&self, index: usize) -> Result<&str, HostError> {
        let value = self.value(index)?;
        value
            .as_str()
            .ok_or_else(|| mistyped(index, "a string", value))
    }

    pub fn number(&self, index: usize) -> Result<f64, HostError> {
        let value = self.value(index)?;
        value
            .as_number()
            .ok_or_else(|| mistyped(index, "a number", value))
    }

    /// A number that has to be whole — an identifier, a count, an index.
    pub fn integer(&self, index: usize) -> Result<i64, HostError> {
        let number = self.number(index)?;
        if number.fract() != 0. {
            return Err(HostError::new(format!(
                "argument {} must be a whole number, got {number}",
                index + 1
            )));
        }
        Ok(number as i64)
    }

    pub fn boolean(&self, index: usize) -> Result<bool, HostError> {
        let value = self.value(index)?;
        value
            .as_bool()
            .ok_or_else(|| mistyped(index, "a boolean", value))
    }
}

fn mistyped(index: usize, expected: &str, got: &HostValue) -> HostError {
    HostError::new(format!(
        "argument {} must be {expected}, got {}",
        index + 1,
        got.describe()
    ))
}

/// A host function said no.
///
/// It carries a sentence and nothing else: the engine adds the module and
/// function names when it turns this into a script exception, so a host writing
/// a function never repeats its own name in the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError {
    message: String,
}

impl HostError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HostError {}

impl From<String> for HostError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for HostError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// What a host function returns.
pub type HostResult = Result<HostValue, HostError>;

/// `Rc` rather than `Box` because the registry is handed out by clone on every
/// call — see `modules` — and a boxed closure cannot be shared that way.
type HostFunction = Rc<dyn Fn(&HostArguments) -> HostResult>;

/// The work an asynchronous host function hands back.
///
/// `Send` because it is driven on the background executor, and `'static`
/// because it outlives the call that produced it. Both fall out of the split
/// described on [`HostModule::async_function`]: whatever the future needs from
/// the `App` was copied out before it was built.
pub type HostFuture = Pin<Box<dyn Future<Output = HostResult> + Send>>;

/// Builds one asynchronous call. Fallible so that argument checking can refuse
/// before any work is scheduled.
type HostAsyncFunction = Rc<dyn Fn(&HostArguments) -> Result<HostFuture, HostError>>;

/// One registered module: a name and the functions under it.
#[derive(Clone)]
pub struct HostModule {
    name: String,
    /// Sorted, so the "it provides: …" line in a diagnostic reads the same on
    /// every run regardless of registration order.
    functions: BTreeMap<String, HostFunction>,
    /// Sorted alongside `functions`, and disjoint from it: one name is either
    /// synchronous or asynchronous, never both.
    async_functions: BTreeMap<String, HostAsyncFunction>,
    /// The module's TypeScript face, if the host wrote one. See
    /// [`HostModule::declarations`].
    declarations: Option<String>,
}

/// The dispatch hook, run at the top of every registered host function on the calling
/// thread. The host (bir-app-api) installs it once per process: it is the drain of the
/// machine's menu-and-chrome notices onto the engine thread (F11). The default does nothing.
static DISPATCH_HOOK: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

/// Installs the dispatch hook. The first install wins; a test process installs once.
pub fn set_dispatch_hook(hook: Box<dyn Fn() + Send + Sync>) {
    let _ = DISPATCH_HOOK.set(hook);
}

fn dispatch_drain() {
    if let Some(hook) = DISPATCH_HOOK.get() {
        hook();
    }
}

impl HostModule {
    /// Names a module a script may import.
    ///
    /// The name is the specifier: `HostModule::new("workspace")` is what makes
    /// `import { … } from "workspace"` resolve. A name the runtime already owns
    /// is refused by [`Self::validate`], which `gpui_shell::export_module` calls —
    /// see [`RESERVED_SPECIFIERS`].
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            functions: BTreeMap::new(),
            async_functions: BTreeMap::new(),
            declarations: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Registers one function.
    ///
    /// The body must not call back into the script engine; see the module
    /// header. It may read and write host state, and may ask a view to
    /// re-render — the notification is delivered after the call unwinds.
    pub fn function(
        mut self,
        name: impl Into<String>,
        body: impl Fn(&HostArguments) -> HostResult + 'static,
    ) -> Self {
        let name = name.into();
        self.async_functions.remove(&name);
        self.functions.insert(
            name,
            Rc::new(move |arguments| {
                dispatch_drain();
                body(arguments)
            }),
        );
        self
    }

    /// Registers one function whose work runs off the main thread.
    ///
    /// The script gets a promise. What it awaits is driven on GPUI's background
    /// executor, so a slow call — a request, a query, a large file — does not
    /// hold the thread that renders.
    ///
    /// ```no_run
    /// # use bir_host_shim::{HostModule, HostValue};
    /// HostModule::new("db")
    ///     .declarations("export function query(sql: string): Promise<unknown[]>;")
    ///     .async_function("query", |arguments| {
    ///         // Synchronous half: on the main thread, so it may read host
    ///         // state, and refusing here costs nothing.
    ///         let sql = arguments.string(0)?.to_owned();
    ///
    ///         // Asynchronous half: on the background executor.
    ///         Ok(Box::pin(async move {
    ///             let _ = sql;
    ///             Ok(HostValue::Null)
    ///         }))
    ///     });
    /// ```
    ///
    /// # The two halves, and why the split is the design
    ///
    /// The closure runs on the main thread, inside the calling script's scope,
    /// and returns the future. So the *arguments* can be checked against host
    /// state — `gpui_shell::with_current_app` works here, exactly as it does in a
    /// synchronous function — and anything the work needs is copied out before
    /// the future is built.
    ///
    /// The future is then `Send + 'static` and driven on another thread, where
    /// there is no `App` and no `Ctx` to reach for. That is not a restriction
    /// bolted on: it is the same rule the module header states for synchronous
    /// functions — a host function may not re-enter the engine — made physical.
    /// A synchronous body is held to it by a runtime guard; an asynchronous one
    /// simply cannot express the violation.
    ///
    /// # Cancellation
    ///
    /// If the view that made the call goes away, or its application is
    /// reloaded, the promise stays pending for ever and the script's
    /// continuation never runs. No error is invented for code that was asked to
    /// stop — the same answer `cx.sleep` gives.
    ///
    /// # Declaring it
    ///
    /// Write the return type as a `Promise` in [`Self::declarations`]. The
    /// registry checks that the *names* on both sides agree; it does not read
    /// signatures, so nothing stops a declaration from omitting the `Promise`
    /// and nothing catches it later either.
    pub fn async_function<F>(
        mut self,
        name: impl Into<String>,
        body: impl Fn(&HostArguments) -> Result<F, HostError> + 'static,
    ) -> Self
    where
        F: Future<Output = HostResult> + Send + 'static,
    {
        let name = name.into();
        // One name, one kind. Registering over the other table rather than
        // beside it keeps `function_names` a single list and stops a call
        // having to decide which of two entries it meant.
        self.functions.remove(&name);
        self.async_functions.insert(
            name,
            Rc::new(move |arguments| {
                let future = body(arguments)?;
                Ok(Box::pin(future) as HostFuture)
            }),
        );
        self
    }

    /// Describes this module in TypeScript, for the generated declarations.
    ///
    /// The body of a `declare module`, so it may carry helper types alongside
    /// the exports:
    ///
    /// ```no_run
    /// # use bir_host_shim::{HostModule, HostValue};
    /// HostModule::new("market")
    ///     .function("quotes", |_| Ok(HostValue::Null))
    ///     .declarations(
    ///         r#"
    ///         export interface Quote { symbol: string; last: string }
    ///         export function quotes(): Quote[];
    ///         "#,
    ///     );
    /// ```
    ///
    /// Writing it here rather than in a `.d.ts` beside the script is what keeps
    /// the two halves one thing. A `.d.ts` is a second file, in a second
    /// language, with nothing holding it to the registry; here, [`Self::validate`]
    /// checks that every registered function is declared and every declared
    /// function is registered, so renaming one half fails at start-up instead of
    /// at the call site.
    ///
    /// Declaring nothing is allowed and costs only precision: an undeclared
    /// module is emitted with `(...args: any[]) => any` signatures, which still
    /// checks the module name and every export name.
    pub fn declarations(mut self, typescript: impl Into<String>) -> Self {
        self.declarations = Some(typescript.into());
        self
    }

    /// The TypeScript body [`Self::declarations`] was given.
    pub fn declared(&self) -> Option<&str> {
        self.declarations.as_deref()
    }

    /// Every name this module exports, synchronous and asynchronous together,
    /// sorted.
    ///
    /// One list because the script sees one list: both kinds are `export const`
    /// in the generated module, and which is which is the return type's
    /// business, not the import's.
    pub fn function_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .functions
            .keys()
            .chain(self.async_functions.keys())
            .map(String::as_str)
            .collect();
        names.sort_unstable();
        names
    }

    /// Every exported function with its kind, sorted by name. Not in
    /// gpui_shell: the bir-shell adapter and the NSH door read the registry
    /// through it.
    pub fn functions(&self) -> Vec<(&str, FunctionKind)> {
        let mut functions: Vec<(&str, FunctionKind)> = self
            .functions
            .keys()
            .map(|name| (name.as_str(), FunctionKind::Sync))
            .chain(
                self.async_functions
                    .keys()
                    .map(|name| (name.as_str(), FunctionKind::Async)),
            )
            .collect();
        functions.sort_unstable_by(|a, b| a.0.cmp(b.0));
        functions
    }

    pub fn has(&self, function: &str) -> bool {
        self.functions.contains_key(function) || self.async_functions.contains_key(function)
    }

    fn has_export(&self, name: &str) -> bool {
        self.has(name)
    }

    /// Whether this name answers with a promise.
    ///
    /// Read by the engine when it builds the binding, because the two return
    /// different things to the script and the difference cannot be deferred to
    /// call time: a promise borrows the context lifetime.
    pub fn is_async(&self, function: &str) -> bool {
        self.async_functions.contains_key(function)
    }

    /// Refuses a name the runtime owns, and a TypeScript face that disagrees
    /// with what was registered.
    ///
    /// Called by `gpui_shell::export_module` and `gpui_shell::policy::Policy::with_host_module`,
    /// which is where a host still has somewhere to put the sentence. Building
    /// a module is infallible on purpose: a builder that returned a `Result`
    /// from every step could not be a chain.
    pub fn validate(&self) -> Result<(), HostError> {
        if RESERVED_SPECIFIERS.contains(&self.name.as_str()) {
            return Err(HostError::new(format!(
                "`{}` is one of the runtime's own module names and cannot be \
                 registered: a script importing it reaches the runtime, never this \
                 module. The reserved names are: {}",
                self.name,
                list(RESERVED_SPECIFIERS)
            )));
        }
        self.check_declarations()
    }

    /// Compares the declared exports with the registered ones.
    ///
    /// Only `export function` and `export const` at the start of a line are
    /// read as exports — enough to name every function a module can have, and
    /// deliberately not a TypeScript parser: anything it cannot recognize it
    /// leaves alone, so a helper type or a comment is never mistaken for a
    /// missing function.
    fn check_declarations(&self) -> Result<(), HostError> {
        let Some(declarations) = self.declared() else {
            return Ok(());
        };

        let declared: Vec<&str> = declarations
            .lines()
            .filter_map(|line| {
                let line = line.trim_start();
                let rest = line
                    .strip_prefix("export function ")
                    .or_else(|| line.strip_prefix("export declare function "))
                    .or_else(|| line.strip_prefix("export const "))?;
                let end =
                    rest.find(|it: char| !it.is_ascii_alphanumeric() && it != '_' && it != '$');
                Some(&rest[..end.unwrap_or(rest.len())])
            })
            .collect();

        let missing: Vec<&str> = self
            .function_names()
            .into_iter()
            .filter(|name| !declared.contains(name))
            .collect();
        let extra: Vec<&str> = declared
            .iter()
            .copied()
            .filter(|name| !self.has_export(name))
            .collect();

        if missing.is_empty() && extra.is_empty() {
            return Ok(());
        }

        let mut message = format!(
            "HostModule `{}` declares a different set of exports than it registers",
            self.name
        );
        if !missing.is_empty() {
            let _ = write!(message, "; registered but not declared: {}", list(&missing));
        }
        if !extra.is_empty() {
            let _ = write!(message, "; declared but not registered: {}", list(&extra));
        }
        Err(HostError::new(message))
    }

    /// Calls one synchronous function, reporting an unknown name against what
    /// this module actually provides.
    pub fn call(&self, function: &str, arguments: &HostArguments) -> HostResult {
        let Some(body) = self.functions.get(function) else {
            return Err(self.no_such_function(function));
        };
        body(arguments)
    }

    /// Starts one asynchronous call, returning the work to be driven.
    ///
    /// The argument checking inside `body` happens here, on the main thread and
    /// inside the caller's scope, which is why this is fallible separately from
    /// the future it returns.
    pub fn begin(
        &self,
        function: &str,
        arguments: &HostArguments,
    ) -> Result<HostFuture, HostError> {
        let Some(body) = self.async_functions.get(function) else {
            return Err(self.no_such_function(function));
        };
        body(arguments)
    }

    fn no_such_function(&self, function: &str) -> HostError {
        HostError::new(format!(
            "HostModule `{}` has no function `{function}`; it provides: {}",
            self.name,
            list(&self.function_names())
        ))
    }
}

/// The specifiers a host may not register, because something else answers them.
///
/// Two groups, and the reason is the same for both: the resolver chain reaches
/// them before it reaches this registry, so a HostModule wearing one of these
/// names is unreachable rather than overriding. The engine asserts this list
/// against its own resolvers, so a module added there and not here is a failing
/// test rather than a name a host can quietly lose.
pub const RESERVED_SPECIFIERS: &[&str] = &[
    // The runtime's own modules.
    "gpui-kit",
    "gpui",
    "gpui-base",
    "gpui-component",
    "gpui-shell",
    "gpui-fps",
    // The Standard Runtime.
    "buffer",
    "console",
    "crypto",
    "fs/promises",
    "net",
    "os",
    "path",
    "process",
    "url",
    "websocket",
    "zlib",
];

fn list(names: &[&str]) -> String {
    if names.is_empty() {
        "nothing".to_owned()
    } else {
        names.join(", ")
    }
}

#[cfg(test)]
mod tests;
