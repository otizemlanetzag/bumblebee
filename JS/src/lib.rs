//! Bumblebee JavaScript execution layer, powered by QuickJS through rquickjs.
use rquickjs::{Context, Runtime as QuickJsRuntime};

/// A persistent JavaScript runtime and global context for one Bumblebee realm.
pub struct Runtime {
    _runtime: QuickJsRuntime,
    context: Context,
}

impl Runtime {
    /// Create a new JavaScript runtime with the standard ECMAScript intrinsics.
    pub fn new() -> Result<Self, String> {
        let runtime = QuickJsRuntime::new().map_err(|error| format!("QuickJS initialization failed: {error}"))?;
        // Keep a finite default heap limit; browser integration can tune this per realm.
        runtime.set_memory_limit(256 * 1024 * 1024);
        let context = Context::full(&runtime).map_err(|error| format!("QuickJS context creation failed: {error}"))?;
        Ok(Self { _runtime: runtime, context })
    }

    /// Evaluate JavaScript in the persistent global context.
    /// Variables and functions remain available to subsequent calls.
    pub fn execute(&self, source: &str) -> Result<String, String> {
        self.context.with(|ctx| {
            ctx.eval::<rquickjs::Value<'_>, _>(source)
                .map(|value| format!("{value:?}"))
                .map_err(|error| format!("JavaScript error: {error}"))
        })
    }

    /// Evaluate a script and return its JavaScript string conversion.
    pub fn execute_string(&self, source: &str) -> Result<String, String> {
        self.context.with(|ctx| {
            ctx.eval::<String, _>(format!("String((()=>{{\n{source}\n}})())"))
                .map_err(|error| format!("JavaScript error: {error}"))
        })
    }

    /// Trigger QuickJS's cycle collector.
    pub fn collect_garbage(&self) {
        self.context.with(|ctx| ctx.run_gc());
        self._runtime.run_gc();
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;

    #[test]
    fn supports_modern_javascript_features() {
        let runtime = Runtime::new().unwrap();
        let result = runtime.execute("const twice = x => x * 2; twice(21)").unwrap();
        assert!(result.contains("42"), "unexpected result: {result}");
    }

    #[test]
    fn preserves_global_state_between_scripts() {
        let runtime = Runtime::new().unwrap();
        runtime.execute("globalThis.answer = 40").unwrap();
        let result = runtime.execute("answer + 2").unwrap();
        assert!(result.contains("42"), "unexpected result: {result}");
    }

    #[test]
    fn reports_javascript_exceptions() {
        let runtime = Runtime::new().unwrap();
        assert!(runtime.execute("throw new Error('test failure')").is_err());
    }
}
