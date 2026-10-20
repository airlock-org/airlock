//! Interpolate named build dependencies into shell assets without discarding Nix string contexts.

use rusix::interop::raw::NixValue;

/// Replace `@name@` markers in a shell asset with deferred text or package paths.
/// Nix performs replacement during evaluation and retains dependencies on package outputs.
/// Shell argument quoting belongs to the caller because some markers form executable paths
/// while others represent complete, potentially untrusted arguments.
pub fn substitute<const N: usize>(script: &str, bindings: [(&str, NixValue); N]) -> NixValue {
    NixValue::builtin("replaceStrings")
        .call(NixValue::list(
            bindings.iter().map(|(name, _)| format!("@{name}@").into()),
        ))
        .call(NixValue::list(
            bindings.into_iter().map(|(_, value)| value.to_text()),
        ))
        .call(script)
}
