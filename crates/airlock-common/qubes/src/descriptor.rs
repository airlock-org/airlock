//! Supply native methods to ordinary Python subclasses without generating Python source.
//! Qubes still constructs its own base objects and discovers their event metadata.

use pyo3::{
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError},
    prelude::*,
    types::{PyDict, PyTuple},
};

/// Native implementation of a named method; positional arguments include its Python instance.
pub type Body =
    for<'py> fn(&str, &Bound<'py, PyTuple>, Option<&Bound<'py, PyDict>>) -> PyResult<Py<PyAny>>;

/// A Python method descriptor whose implementation lives in Rust.
#[pyclass(dict, module = "airlock_qubes")]
struct Method {
    // User-visible method identity, also used for native dispatch and Qubes diagnostics.
    name: String,
    // Native implementation; Python never supplies arbitrary executable source.
    body: Body,
    // Async Qubes methods defer their work until upstream actually awaits them.
    deferred: bool,
}

#[pymethods]
impl Method {
    /// Expose a normal method name to Python introspection and event diagnostics.
    #[getter(__name__)]
    fn name(&self) -> &str {
        &self.name
    }

    /// Bind an instance exactly as an ordinary Python function descriptor does.
    fn __get__(
        slf: Py<Self>,
        py: Python<'_>,
        instance: Bound<'_, PyAny>,
        _owner: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if instance.is_none() {
            return Ok(slf.into_any());
        }
        Ok(py
            .import("types")?
            .getattr("MethodType")?
            .call1((slf, instance))?
            .unbind())
    }

    /// Dispatch immediately or return a coroutine whose first resume performs the native work.
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        if !self.deferred {
            return (self.body)(&self.name, args, kwargs);
        }

        Ok(Py::new(
            args.py(),
            Deferred {
                name: self.name.clone(),
                body: self.body,
                args: args.clone().unbind(),
                kwargs: kwargs.map(|value| value.clone().unbind()),
                finished: false,
            },
        )?
        .into_any())
    }
}

/// A single-use coroutine recognized by asyncio and Qubes' coro_maybe helper.
#[pyclass(module = "airlock_qubes")]
struct Deferred {
    // Operation held until Qubes resumes this coroutine on its own event loop.
    name: String,
    // Native body shared with synchronous descriptors.
    body: Body,
    // Retained upstream objects, including the pool or volume instance.
    args: Py<PyTuple>,
    // Named arguments retained without changing their upstream types.
    kwargs: Option<Py<PyDict>>,
    // Coroutines cannot be restarted after success, failure or cancellation.
    finished: bool,
}

#[pymethods]
impl Deferred {
    /// Return this coroutine's iterator to Python's await machinery.
    fn __await__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// A coroutine iterator resumes the same deferred native operation.
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Resume a freshly awaited coroutine with the required initial None value.
    fn __next__(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.send(py, py.None())
    }

    /// Perform exactly one native operation and return its result through StopIteration.
    fn send(&mut self, py: Python<'_>, value: Py<PyAny>) -> PyResult<Py<PyAny>> {
        if self.finished {
            return Err(PyRuntimeError::new_err(
                "cannot reuse an awaited Airlock coroutine",
            ));
        }
        if !value.bind(py).is_none() {
            return Err(PyTypeError::new_err(
                "cannot send a non-None value to a new coroutine",
            ));
        }
        self.finished = true;

        let result = (self.body)(
            &self.name,
            self.args.bind(py),
            self.kwargs.as_ref().map(|value| value.bind(py)),
        )?;
        Err(PyStopIteration::new_err((result,)))
    }

    /// Propagate cancellation or another upstream exception without starting native work.
    #[pyo3(signature = (error, value=None, _traceback=None))]
    fn throw(
        &mut self,
        error: Bound<'_, PyAny>,
        value: Option<Bound<'_, PyAny>>,
        _traceback: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        self.finished = true;
        let error = if error.is_callable() {
            match value {
                Some(value) => error.call1((value,))?,
                None => error.call0()?,
            }
        } else {
            error
        };
        Err(PyErr::from_value(error))
    }

    /// Discard an unawaited operation without executing it.
    fn close(&mut self) {
        self.finished = true;
    }
}

/// Construct a method descriptor, retaining normal binding and asyncio coroutine semantics.
pub fn method<'py>(
    py: Python<'py>,
    name: &str,
    body: Body,
    deferred: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if deferred {
        py.import("collections.abc")?
            .getattr("Coroutine")?
            .call_method1("register", (py.get_type::<Deferred>(),))?;
    }
    Ok(Py::new(
        py,
        Method {
            name: name.into(),
            body,
            deferred,
        },
    )?
    .into_bound(py)
    .into_any())
}

/// Define a real Python subclass using an upstream class and native descriptors.
pub fn subclass<'py>(
    py: Python<'py>,
    name: &str,
    base: Bound<'py, PyAny>,
    namespace: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    namespace.set_item("__module__", "airlock_qubes")?;
    py.import("builtins")?
        .getattr("type")?
        .call1((name, (base,), namespace))
}

/// Wrap a native getter with Python's standard property descriptor.
pub fn property<'py>(py: Python<'py>, name: &str, body: Body) -> PyResult<Bound<'py, PyAny>> {
    py.import("builtins")?
        .getattr("property")?
        .call1((method(py, name, body, false)?,))
}
