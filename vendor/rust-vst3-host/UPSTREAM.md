# Upstream Source

Repository: https://github.com/HelgeSverre/rust-vst3-host
Revision: `6c64cdffc6740cce0b4c5b6916724697b0ef9ff8` (vst3-host 0.11.1).

The library, its examples/tests, deterministic test plugin, bundle script, and MIT license are copied from the reviewed revision. The verified state-restore patch and the editor-close resize patch are applied; see [patch documentation](../../patches/vst3-host/README.md). The inspector application and upstream workspace manifest are omitted. The Clef root workspace supplies the original shared dependency declarations. No upstream messages or publishing occurred.

The test-plugin crate additionally carries the opt-in Clef latency fixture variant
recorded as patch0003 in the same patch documentation. It is not production host
code; feature-disabled builds retain the original TestSynth behavior.
