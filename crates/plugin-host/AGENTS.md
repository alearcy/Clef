# Plugin Hosting

- Preserve bounded IPC, scanner timeouts, process isolation, and quarantine enforcement.
- Follow each format's threading and lifecycle rules. Keep original editors on the
  worker's eframe path. Preserve the Steinberg SDK bridge for VST3 and Clack for CLAP.
- For native changes, follow the [native guidelines](../../native/AGENTS.md).
