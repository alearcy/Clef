---
name: Sweep bar
kind: component
reference: composer::style::paintSweepBar, composer::style::sweepSeconds
status: shared
impl: planned
tokens: [field, accent]
used-in: [startup, ara-editor]
snapshot: pending
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

# Sweep bar

## Purpose
Indeterminate progress for work whose length is not known: the session loading at launch (D97) and Render ARA to
Audio (D87).

## Anatomy
A 4-px `field` track with 2-px corners; a highlight 30 % of its width, `accent` fading to nothing at both ends,
crosses it left to right once every 1.6 s (`sweepSeconds`), eased at both ends.

## Behaviour
The owner repaints only the bar, 60 times a second, and passes the phase (0…1 through one sweep). It pauses while
the message thread is blocked.

## Do / Don't
- Don't use it where a proportion is known (an import, a stretch being prepared): show that proportion.

## Code
```cpp
style::paintSweepBar(g, bar, std::fmod(seconds, style::sweepSeconds) / style::sweepSeconds);
```
