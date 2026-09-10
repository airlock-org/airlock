# Runtime Layer

Holds the ongoing state Nixome operates after boot.

`config-staging/` is the default runtime configuration candidate. It is the
edit surface copied into the config authority workflow.

`control-tools/` contains the packages and scripts that apply, archive, import,
export, inspect, and test runtime state.
