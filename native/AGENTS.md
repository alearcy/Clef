# Native Bridge Guidelines

Applies to `native/` and changes to its Rust FFI callers.

- Extend the existing `clef_vst3` bridge built against `vendor/vst3sdk` for VST3 hosting.
- Keep the C ABI small. Define handle ownership, lifetimes, thread affinity, and failure behavior
  at the boundary; prevent C++ exceptions and Rust panics from crossing it.
- Preserve SDK threading and lifecycle rules inside the worker. Keep control, editor, and
  teardown operations off the device callback.
- Use native view glue only to attach and detach original plugin editors using the parent view
  supplied by eframe. Keep platform lifecycle hooks scoped to their platform responsibilities.
