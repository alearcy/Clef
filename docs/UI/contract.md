---
name: UI implementation contract
kind: contract
version: 1.0.0
updated: 2026-11-17
---

# UI implementation contract

How an approved Clef design becomes Rust/egui code that two people — and two agents — build
the same way. Read [README](README.md) and [foundations](foundations.md) first, then this file,
then only the surface and the primitives the change touches.

## The failure this prevents

The legacy library describes pixels in prose: "a solid `panel` box, 560 wide, a 1-pixel `divider`
edge, padded 16, the warning glyph at 16, 11-px uppercase captions". That sentence does not
determine a single pixel. An agent filling the gaps reaches for the cheapest available default —
`egui::Window`, `ui.button`, Ubuntu-Light, an 18-point heading — and a different agent fills them
differently. More adjectives do not fix this. The decision has to move into three things a machine
can read exactly: a **token**, a **signature**, and a **golden image**.

## Four layers

| Layer | Lives in | Contains | May not contain |
| --- | --- | --- | --- |
| Tokens | `crates/ui/src/theme.rs`, [foundations](foundations.md) | colours, metrics, text styles, font loading | layout, behaviour |
| Primitives | `crates/ui/src/primitives/*.rs`, one doc each | one visual and behavioural unit, every state, its painter | composition, application state |
| Surfaces | `crates/ui/src/lib.rs` (or `crates/ui/src/surfaces/`), one view doc each | composition, constraints, state-to-primitive mapping | painting, colour literals, ad-hoc sizes |
| Evidence | `crates/ui/src/gallery.rs`, `docs/UI/snapshots/egui/` | one render per primitive per state | claims not backed by an image |

Dependencies point downward only. A primitive never knows about a surface; a surface never paints.

## Token contract

- One role, one Rust constant. `foundations.md` and `theme.rs` carry the same names and values.
- No `Color32::from_rgb`, no `RichText::size`, no pixel constant outside `theme.rs`. A primitive
  may compute from tokens (for example `CONTROL_HEIGHT - 2.0`), never invent a new value.
- Text is never styled ad hoc: `theme::text::{HEADING, BODY, SMALL, CAPTION, TINY, MENU}` map to
  egui `TextStyle`s configured once at startup. A raw `RichText::size(13.0)` in a surface is a
  defect even when the number is right.
- Fonts: the system SF Pro is loaded at startup as `FontFamily::Name("sf-pro")`; egui's default
  font is a development fallback only, flagged in the gallery header, never shipped.
- A test parses the tables in `foundations.md` and fails when a value differs from `theme.rs`
  (`theme::tests` in `crates/ui/src/theme.rs`, T18), and a second test rejects `Color32::from_rgb`
  outside `theme.rs`. The document stays human-readable; the test makes it non-optional.

## Primitive contract

Each primitive file (`docs/UI/components/<id>.md`) must contain these sections. A primitive is
not "done" until every one is filled and the gallery case renders.

| Section | Content |
| --- | --- |
| `api` | exact Rust path, constructor or function signature, builder options, return type (`Response`), and the accesskit name source |
| `anatomy` | a table of regions: region, size, fill token, edge token, text style. Every region named in code appears here |
| `states` | a matrix with one mandatory row per state: rest, hover, pressed, focused, disabled, selected or lit when applicable, unavailable. Each row states what changes in fill, edge, text/glyph and cursor |
| `behaviour` | pointer, keyboard, scroll, tooltip, command binding, focus order, disabled semantics. What the widget does, not how it looks |
| `tokens` | the colour, metric and text roles used; no others may appear in the code |
| `usage` | a compiling snippet copied from a real call site; the gallery uses the same call |
| `evidence` | gallery case ids, snapshot paths, test names |
| `do / don't` | rules a reviewer checks in a diff, stated as falsifiable conditions |
| `impl` | `planned`, `app` or `gallery`; mirror of the frontmatter |

### Example: text button

The example below is the target format written before the first batch; the implemented
[text button](components/text-button.md) follows it with a builder (`TextButton::new(..).show(ui)`)
and leaves command dispatch to the caller.

```yaml
---
name: Text button
kind: component
status: shared
impl: gallery
api: ui::primitives::text_button::TextButton
reference: ComposerStyleKit drawButtonBackground (C++, not in this repository)
tokens: [field, peacock, text, disabled, controlEdge, divider]
used-in: [multitrack-session, settings, startup]
snapshot: snapshots/egui/text-button.png
evidence: [gallery_text_button, tests/ui_text_button.rs]
---
```

**api**

```rust
pub struct TextButton<'a> {
    label: &'a str,
    kind: ButtonKind,        // Secondary | Primary
    command: Option<Command>,
    enabled: bool,
}

impl TextButton<'_> {
    pub fn new(label: &str) -> Self;
    pub fn primary(self) -> Self;
    pub fn command(self, command: Command) -> Self;
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response;
}
```

**anatomy**

| Region | Size | Fill | Edge | Text |
| --- | --- | --- | --- | --- |
| body | height 24 (controlHeight), min width 44, horizontal padding 8 | `field` (secondary) / `peacock` (primary) | none at rest | `text` / foreground on fill |
| body, raised | 2 shorter than bounds | as above | 1-px top light, 2-px soft shadow | unchanged |
| label | centred | — | — | `theme::text::BODY` |

**states**

| State | Fill | Edge | Text | Notes |
| --- | --- | --- | --- | --- |
| rest | field / peacock | top light + shadow | text / foreground_on(peacock) | |
| hover | field lifted | none | unchanged | the body lifts to `field`; never a new border |
| pressed | unchanged | drops top light | unchanged | reads pushed in, size unchanged |
| focused | unchanged | `accent` 1 px | unchanged | keyboard only; click focus draws none |
| disabled | `panel` | `divider` | `disabled` | semantically disabled; no pointer |
| lit (toggle variant) | `peacock` | drops top light | foreground_on(peacock) | used only where the component is a toggle |

**behaviour.** Click invokes `command` through the shared registry; no private action. Space and
Return activate when focused. Tooltip is `"{title} ({shortcut})"` when a shortcut exists. Disabled
buttons still expose their label to accesskit. `Primary` appears at most once per surface.

**usage**

```rust
if TextButton::new("Add Instrument Track").command(Command::AddTrack).show(ui).clicked() {
    dispatch(Command::AddTrack);
}
```

**do / don't.** Use a [tool button](components/tool-button.md) when a glyph says it; never draw a
one-off `egui::Button` in a surface; never colour a text button with a track colour.

## Surface contract

A view doc (`docs/UI/views/<id>.md`) describes composition only. It must contain:

1. **Purpose and states** — which states exist (empty, loading, ready, error, long, narrow) and
   what changes in the composition for each.
2. **Layout tree** — constrained regions, not screenshots:

   ```
   surface multitrack-session (1024 x 640, min 760 x 480)
     column gap=12 pad=12
       row gap=12 hug: heading("Tracks (3)") + text_button("Add Instrument Track")
       row gap=12 fill
         column width=320 scroll: track_list (rows 44)
         column fill scroll: track_details (wrap)
       row hug: status_text
   ```

   Vocabulary: `column`, `row`, `stack`, `grid`, `scroll`, `fill`, `hug`, `wrap`, `fixed(n)`.
   Every number is a token or is justified in the doc; `fixed` is the exception and states why.
3. **Primitives table** — id, role on this surface, command bound, states shown.
4. **Responsive rules** — what shrinks, wraps, scrolls or disappears at 760 points; overflow is a
   defect, never a clipped control.
5. **Acceptance checklist** — every primitive used is `gallery`; a golden exists for each surface
   state; no raw egui widget in the surface code; no literal colour, size or font; accesskit names;
   keyboard path; docs and index updated in the same change.

## Rules for agents (human, Claude, Codex)

1. Read `README.md`, `foundations.md`, this file, the surface, and only the primitives it lists.
2. Compose primitives. `egui::Button`, `Label`, `ComboBox`, `TextEdit`, `Window`, `Modal` and raw
   painters are allowed **only** inside `crates/ui/src/primitives`.
3. If a needed primitive is `planned`, implement it with its doc and golden in the same change, or
   stop and ask. Never substitute a raw widget "just for now": that is how the current surface
   happened.
4. Never introduce a colour, size, radius or font outside `theme.rs`.
5. Never introduce a visual pattern that has no contract entry and golden. Novelty is a proposal,
   not an implementation detail.
6. Done means: gallery case updated, snapshot committed, tests green, primitive and surface docs
   plus `index.md` updated. A green `cargo test` without an image is not visual acceptance.
7. When the design is silent, ask. Do not infer taste.
8. C++/JUCE/Tracktion symbols are not APIs. That code is not in this repository.

## Interaction frame regressions

For changes that affect these interactions, check the frames around the state transition and
record the relevant evidence in the task specification. See [T25](../specs/T25-ui-flicker.md).

- **Range editing.** Keep a valid preview visible through pointer release and session
  acknowledgment. Cancel, an invalid edit, and rejection restore the committed range. Compare
  the drag, release, and pending frames so a one-frame reappearance of the old range fails.
- **Buttons and transport.** Preserve the appearance of unrelated available controls during
  pointer presses, releases, and session or audio-engine acknowledgments. Keep the execution
  gates and show longer waits through status feedback. Verify sidebar and transport buttons
  with audio enabled and playback stopped. Stop should remain dim during unrelated updates and
  become available for pending Play, count-in, and playback.
- **Plugin editor presentation.** Attach, size, and prepare the embedded editor before revealing
  its window. Cancel a pending reveal when closing, hiding, or handling an open failure. Check
  first open and reopen on macOS with an affected plugin and a control plugin when available.
  Pocket Strings VST3 is the T25 reproduction case.

Use frame-level assertions for deterministic UI state and live observation or a recording for
transient compositor behavior. A settled screenshot covers the final state only.

## Dense editing surfaces

These rules apply to the timeline, arrangement, and piano roll:

- Use one viewport transform for drawing and hit-testing. Query items intersecting the visible
  time and lane ranges, including clips or notes that start before the visible range.
- Keep playhead, hover, and drag previews separate from static geometry. Request repaints while
  the surface is changing. Keep controls and accessibility within the existing UI contract.
- Start with the existing egui painter path under the primitive contract. If measurements justify
  caching, key reusable geometry by the source revision and the render inputs it actually depends
  on. Invalidate affected tracks or clips independently and bound cache memory.
- Profile playback, scroll, and zoom before considering tiles, persistent GPU resources, or a
  renderer change. [T19](../specs/T19-performance.md) owns app-wide performance verification.

## Change protocol

1. Written proposal: layout tree, primitive list, states, what is out of scope.
2. User approval, recorded in the task spec.
3. Tokens and primitives first, with gallery cases and goldens (a batch of the 5–8 the surface
   needs, not all 40).
4. Surface composition, then its state snapshots.
5. Docs and index in the same change; the task spec links the snapshot.

## Gallery and goldens

- `crates/ui/src/gallery.rs` registers every primitive and every state under a stable case id, rendered
  on a fixed stage (1024 × 768 at 1×, 2048 × 1536 at 2×) with the real theme and SF Pro.
- Outputs go to `docs/UI/snapshots/egui/<case>.png`; a case without an image is `impl: app` at
  best, never `gallery`.
- Cases include the hostile states: long names, disabled, empty, narrow width, unavailable data.
- Shape differences are reviewed side by side; text and anti-aliasing use a perceptual threshold so
  a GPU change does not fail the build. The exact renderer is an open decision (below).
- A surface snapshot is taken at 1024 and 760 points for each state in its doc.

## Migration from the legacy library

- 61 of the 63 files still quote C++/JUCE symbols. Their frontmatter now says `reference:` and their
  `impl` is `planned`; a banner marks them as reference-only. Their prose is design intent.
- A component is migrated when its primitive lands: replace the C++ block with the Rust `api` and
  `usage`, fill the state matrix, add `evidence`, set `impl: gallery`.
- Legacy snapshots in `snapshots/` are illustrations of the C++ implementation. They may guide a
  design discussion but are never a pixel target for egui; only `snapshots/egui/` is.

## Decisions (2026-11-17)

- **Tokens:** Rust constants in `theme.rs` are the source of truth; a parity test compares them with
  the `foundations.md` tables. No JSON and no codegen.
- **Gallery:** an eframe gallery window renders the cases and writes PNGs from
  `ViewportCommand::Screenshot`. No new dependency.
- **First batch:** the eight primitives T04 needs — text button, notice box, list row, selection
  edge, text field, chip, window frame, caption.
  Extended on 2026-11-21 ([T18](../specs/T18-ui-foundation.md) Phase 1 refresh) with tool button,
  menu button and scroll and zoom bars, and with adoption on the existing surfaces.
- **Order:** primitives first, then the T04 surface. Until the batch lands, T04 UI stays functional
  but provisional; no new surface may add raw egui widgets.
- **Theme name:** "Composer Dark" stays as the historical name of the look; Clef is the product.

## Remaining questions

- **SF Pro loading:** whether `/System/Library/Fonts/SFNS.ttf` registers correctly in epaint/ab_glyph
  as a variable font. If not, choose between a macOS CoreText/font-kit extraction and accepting
  Helvetica Neue. This is a spike inside the first batch, not a blocker for the token work.
- **Screenshot determinism:** pin `pixels_per_point = 1.0` and the stage size; verify that the same
  machine produces byte-identical PNGs across runs before adding an automatic diff.

## One-screen checklist

- [ ] Every colour, size and font comes from `theme.rs`.
- [ ] Every control is a primitive in `crates/ui/src/primitives`, not a raw egui widget.
- [ ] Every primitive has a state matrix, a Rust snippet and a golden.
- [ ] The surface doc has a layout tree and a states list.
- [ ] 1024 and 760 points rendered; nothing clipped or overlapping.
- [ ] Long names truncate with a tooltip; unavailable data shows a dash.
- [ ] Keyboard path and accesskit names exist.
- [ ] Docs, index and task spec updated in the same change.
