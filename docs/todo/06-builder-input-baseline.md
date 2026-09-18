# Seed a small build-tool baseline

Use the sys-dev guest-build trial's download list to select a modest baseline
of tools needed for routine configuration changes. Declare it in Nix and seed
its complete dependency closure into the USB's shared pool, extending the
existing frozen `sys-dev-builder` input closure. Avoid seeding every possible
build dependency; uncommon inputs can still be downloaded.

Keep these tools protected by the builder's persistent pool pin, including
while sys-dev is stopped. They need not be installed in sys-dev's or sys-ctrl's
operating-system closure. Preserve the metadata/base identity checks; do not
silently replace the base underneath an existing writable build layer.

Verify on a fresh image that a small guest configuration change reuses the
baseline without downloading or copying those inputs, and that pool cleanup
retains them with sys-dev stopped. Record the added image size and any remaining
downloads. Include no sys-ctrl implementation in this task.
