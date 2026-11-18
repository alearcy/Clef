---
name: Clef UI library
kind: index
version: 2.0.0
updated: 2026-11-17
---

# Clef UI library

The single UI authority for **Clef**, a personal DAW built with **Rust + egui/eframe** for film,
neo-classical and orchestral composers. It is written for any reader, human or AI model, with no
tool-specific syntax.

The design language originated in a C++/JUCE/Tracktion application ("Composer DAW"). **That code is
not in this repository.** Its symbols, paths and screenshots are historical references that
describe intent; they are never APIs, never built, and never a pixel target for Clef. If this file
and a legacy C++ reference disagree, this file wins.

Start at [index.md](index.md) for the map. Read this file, then [foundations](foundations.md),
then [contract](contract.md), then only the component and view files the change touches.

## Authority, in order

1. `crates/ui/src/theme.rs` and `crates/ui/src/primitives/` — the Rust that actually paints. When it exists, it
   wins. **It does not exist yet** (T18): until it does, `crates/ui/src/lib.rs` is the interim implementation,
   and every surface it draws is provisional, not the design.
2. [foundations](foundations.md) — the design values: colours, typography, metrics, depth, states,
   separation. A test compares its tables with `theme.rs`; a disagreement fails the build.
3. Component and view files — the contract for each surface and primitive. The `impl` field says
   whether Rust code exists; prose alone never means "available".
4. [contract](contract.md) — how a design becomes code, and what an agent may and may not do.
5. `docs/specs/` — the history of decisions. A spec may describe something later superseded; this
   library holds the current rule.

Legacy snapshots are illustrations of the C++ implementation. Only `snapshots/egui/` shows Clef.

## Platform

- Clef-owned windows and controls are **egui/eframe** only. Do not create application windows or
  controls with AppKit, `NSWindow` or other native macOS UI libraries.
- Original plugin editors are hosted through the egui windowing path, unchanged; the plugin owns
  those pixels.
- No JUCE, no Tracktion, no C++ UI code in the build. Copying a legacy class or its LookAndFeel is
  a defect even when it looks convenient.
- macOS is the current target; Windows later. Sizes are egui points (logical pixels at scale 1.0).

## Layout

- [foundations.md](foundations.md) — token values and rules. Read it first, always.
- [contract.md](contract.md) — the four layers, the primitive and surface templates, the gallery,
  the rules for agents, and the open decisions.
- `components/` — reusable primitives: buttons, fields, chips, rows, notice box, meters, notes.
- `views/` — composed surfaces: window, transport, track header, mixer, piano roll, quarantine,
  settings. A view lists the primitives it is made of.
- `snapshots/` — illustrations from the legacy C++ kit (reference only).
- `snapshots/egui/` — renders of the real Clef primitives and surfaces (T18).
- [index.md](index.md) — the full navigation map.

## Status and implementation

Every component and view file has two independent facts:

- `status` — design ownership: **shared** — belongs in a primitive, reused everywhere; **app** —
  built for one surface, move it into a primitive when a second surface needs it; **mixed** — a
  primitive paints it, the surface wraps behaviour around it; **deprecated** — do not use.
- `impl` — Rust availability: **planned** — no Clef code exists (the legacy C++ reference may);
  **app** — implemented inline in `crates/ui/src/lib.rs`, provisional; **gallery** — implemented in
  `crates/ui/src/primitives/` with a golden image and tests, reuse it as is.

`status: shared` does not mean the code exists. `impl` answers that question. No component is
`gallery` yet.

## File template

```yaml
---
name: Text button                       # display name
kind: component                         # component | view | foundations | contract | index
status: shared                          # shared | app | mixed | deprecated
impl: gallery                           # planned | app | gallery
api: ui::primitives::text_button::TextButton   # exact Rust path, once it exists
reference: ComposerStyleKit drawButtonBackground  # legacy symbol, optional, never an API
tokens: [field, peacock, text, disabled, controlEdge]
used-in: [multitrack-session, settings, startup]
snapshot: snapshots/egui/text-button.png
evidence: [gallery_text_button, tests/ui_text_button.rs]
---
```

Then these sections, in this order: Purpose · Anatomy · Metrics · States · Behaviour · Do / Don't ·
Usage. The [contract](contract.md#primitive-contract) defines what each section must contain; a
primitive without a state matrix, a Rust usage snippet and a golden is not done.

Legacy files keep their old sections and C++ block until migrated. Their frontmatter already marks
them `reference:` and `impl: planned`, and a banner says so at the top.

## Rules for every UI change

1. **Compose primitives before drawing anything.** A surface may not call `egui::Button`,
   `Label`, `ComboBox`, `TextEdit`, `Window`, `Modal` or a raw painter: those belong inside
   `crates/ui/src/primitives/`. If no primitive fits, add or extend one, with its doc and golden, in the
   same change. Never paint a one-off control.
2. **Colours only through `theme.rs` roles.** Never a literal hex, never a colour absent from
   [foundations](foundations.md). Never `RichText::size`; use the text tokens.
3. **Sizes only through metrics tokens** or values stated in the primitive file. A new recurring
   size becomes a token.
4. **Never separate regions with a rule.** Use spacing and background roles (see foundations,
   Separation).
5. **Every user action goes through the shared command registry.** A button, glyph, chip or menu
   row never owns private behaviour.
6. **Mockup and approval before implementation.** Write the proposal (layout tree, primitives,
   states), get the user's approval, link it from the task spec, and compare a render of the real
   primitive with the reference before claiming visual acceptance. A screenshot taken by hand is
   evidence for review, not a golden.
7. **Update the component or view file and `index.md` in the same change as the code.** A file that
   disagrees with the code is a bug; a green `cargo test` without a snapshot is not visual
   acceptance.
8. **Legacy C++ is reference, not foundation.** Read the prose for intent, translate it, and never
   port its classes, its LookAndFeel or its file layout.

## What done means

For a primitive: `impl: gallery`, a gallery case per state, a committed golden, tests, and the doc
sections filled. For a surface: all its primitives are `gallery`, a layout tree and states list in
its doc, renders at 1024 and 760 points for each state, no clipped or overlapping control, and the
task spec links the evidence.

Until T18 lands there are no gallery-tested shared primitives. In that state a UI change is allowed only as
explicitly provisional work, and the task spec must say so.

## Product context

Clef serves composers of film, neo-classical and orchestral music: MIDI writing, arrangement,
chords and a convenient piano roll come first, audio after. External instruments and effects
(VST3, AU, later CLAP); no built-in plugins for now. Not aimed at trap, hip-hop, DJ or lo-fi work.
The tone is a calm, dense, professional workstation, not a consumer app. The one look is called
**Composer Dark** (historical name); Clef adopts it unchanged.
