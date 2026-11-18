# T18 — egui Design System, Commands, and Accessibility

**Status:** Planning draft — Phase 1 scope proposed 2026-11-17; not approved for implementation.
**Priority:** P2.
**Date:** 2026-11-17
**Task:** [T18](../../TASKS.md)
**Roadmap:** [Post-T03 backlog](../planning/post-T03-backlog.md)
**Depends on:** Ongoing UI work; `docs/UI/` authority.

## Goal

Extract and formalize the UI foundation before the application grows more editing surfaces:
design tokens, reusable egui widgets, menus, focus and keyboard navigation, accessible
names, contrast, scaling, command palette, and shortcut editing. UI documentation must be
updated in the same change.

## Proposed Scope

- Extract T03's colors, metrics, and one-off widgets from `app.rs` into shared modules.
- Implement or consolidate reusable components selected from `docs/UI/components/`.
- Menus, context menus, tooltips, and keyboard accessibility.
- Accessible names and focus states for all controls; no label relying only on color.
- Contrast and scaling behavior, including window resizing.
- Command palette and user-assignable shortcuts through the command registry.
- UI documentation updates and mapping to the egui implementation.

## Out of Scope

A visual redesign; adopting native AppKit controls; web or mobile UI; a full component
inventory unrelated to implemented views.

## Open Questions

- Accessibility target and whether accesskit/screen-reader support is required now.
- Which custom-painted egui controls need accessible fallbacks.
- High-contrast and scaling requirements, and minimum supported window sizes.
- Keyboard-only workflows and focus order for complex views.
- Shortcut conflict policy and whether user remapping is in scope.
- Which `docs/UI` components are translated now versus when a view needs them.
- ~~How the egui implementation is documented relative to the legacy C++ kit.~~ Resolved
  2026-11-17: the C++ kit is reference-only and is not in this repository; the
  [UI contract](../UI/contract.md) defines the egui documentation format.

## UI Proposal Requirement

This task is itself the UI foundation. It still requires a written proposal for any new
or changed surface and user approval before implementation, per `docs/UI/README.md`.

## Documentation normalization (2026-11-17)

Planning work completed before implementation, so the task has an unambiguous starting point:

- `docs/UI/README.md` no longer names C++ as authority. `crates/ui/src/theme.rs` and
  `crates/ui/src/primitives/` are the target authority; `docs/UI/foundations.md` holds the values;
  `crates/ui/src/lib.rs` is explicitly interim until T18 lands.
- `docs/UI/contract.md` defines the four layers (tokens, primitives, surfaces, evidence), the
  required sections of a primitive and a surface document, the gallery and golden policy, the
  rules for agents, and a worked text-button contract.
- `docs/UI/foundations.md` keeps every token value but is Rust-first: Rust constant names, egui
  text styles, system SF Pro loading, no JUCE units.
- All 63 component and view files were normalized: `kit:` became `reference:`, legacy paths became
  `reference-source:`, and every file gained `impl: planned | app | gallery`. 61 files carry a
  reference-only banner.
- `docs/UI/index.md` states that `status` is design ownership and `impl` is availability; no
  component is `gallery` yet.

Decisions taken on 2026-11-17 (recorded in
[contract.md](../UI/contract.md#decisions-2026-11-17)): Rust tokens with a doc-parity test;
eframe screenshots for the gallery; the eight T04 primitives first; primitives before the T04
surface; the theme keeps the name Composer Dark.

## Phase 1 — First primitive batch (proposed, awaiting approval)

**Goal:** put the eight primitives the current T04 surface needs into `crates/ui/src/primitives`, with real tokens,
goldens and tests, before touching the T04 layout. This is the smallest slice that proves the
contract end to end.

**Deliverables**

1. `crates/ui/src/theme.rs` — every colour role from `foundations.md` (28), the window/chrome and roll
   metrics, the six text styles, `foreground_on`, `routing_accent`, `track::COLOURS`, and the SF Pro
   font loader used at startup.
2. `crates/ui/src/paint.rs` — `control_body` (raised/recessed), `selection_edge`, and the shadow helpers.
3. `crates/ui/src/primitives/` — `text_button`, `notice_box`, `list_row`, `selection_edge`, `text_field`,
   `chip`, `window_frame`, `caption`; each with every state, accesskit name and tooltip rule.
4. `src/bin/ui_gallery.rs` — an eframe stage that registers a stable case id per primitive and
   state and writes PNGs to `docs/UI/snapshots/egui/` through `ViewportCommand::Screenshot`.
5. `tests/ui_tokens.rs` — parse the colour, text-style and simple-metric tables in
   `foundations.md` and fail when either side drifts; a second check rejects colour literals outside
   `theme.rs`.
6. Migrated docs for the eight components (`impl: gallery`, Rust API, state matrix, usage,
   evidence) and their rows in `index.md`; the two Rust view notes stay provisional.

**Acceptance**

- `rtk cargo test -p clef --locked` passes, including the token parity and primitive behaviour
  tests; a deliberate hex change in either place fails the parity test.
- `rtk cargo run --bin ui_gallery` writes one image per primitive per state; two runs on the same
  machine produce identical bytes at `pixels_per_point = 1.0`.
- Every primitive doc has the sections required by the contract; no component is marked `gallery`
  without its image.
- The text button, notice box and list row are reviewed side by side with the legacy illustrations
  and accepted by the user as the intended look, not merely "not broken".
- No surface code outside `crates/ui/src/primitives` calls a raw egui control; `crates/ui/src/lib.rs` is unchanged
  except for `App::new` applying the theme and fonts.

**Order:** theme and parity test → selection edge, caption, text button → list row and notice box
(the fault screen) → text field and chip → window frame → gallery pass and doc migration.

**Out of scope:** rewriting the T04 surface (Phase 2); the remaining 32 components; the command
registry extraction; a full accesskit audit; menus, palette, light theme or scaling beyond
`pixels_per_point = 1.0`.

**Risks and open items**

- SF Pro: if `SFNS.ttf` does not register as a usable variable font, choose between a macOS
  CoreText/font-kit extraction and Helvetica Neue; decide before the gallery images are committed.
- `window_frame` is an in-app frame for Clef dialogs, never a repaint of the OS title bar.
- Screenshot byte-determinism must be verified before an automatic diff replaces manual review.

## Verification Intent

Widget behavior and states; focus order; shortcut dispatch; contrast tokens; UI docs match
the implementation; no one-off control introduced where a shared component exists.

## Approval Gate

This document is a planning draft. An interview, an approved specification, and an
approved written UI proposal are required before any implementation.
