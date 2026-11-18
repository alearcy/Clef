# Clef SDK Provenance

VST3 SDK 3.8.1, MIT. Upstream https://github.com/steinbergmedia/vst3sdk
at `3cdf9ca5d1f5b1b21e0a86832aa4abe55607bd96`.
Required submodules are pinned by that commit:

- base: `fcf9da0bd27a16f7f03773a3a39822f28f5c8477`
- cmake: `054c9143cbb8d47fc4694e473f2ee3b4d951a8f5`
- pluginterfaces: `4f547e8e102b47de4a8b8aaf343c73b700786372`
- public.sdk: `586dc5e6c8012c3e4b01c79389375cbe96bdb1da`

No SDK source patch is applied. Clef's C ABI implementation and native build are
in `../native/`; Rust owns eframe windows and the application event loop.
SDK module_mac.mm is compiled only for module loading, not application UI.
No hosting/plugin examples or VSTGUI are built. The wrapper currently targets
macOS arm64, matching the T02 execution host. All upstream licenses are retained.
The user approved this backend amendment on 2026-11-17.

After cloning Clef, run the following commands from its repository root to
initialize the SDK and the components required by the native build:

```sh
git submodule update --init vendor/vst3sdk
git -C vendor/vst3sdk submodule update --init base cmake pluginterfaces public.sdk
```
