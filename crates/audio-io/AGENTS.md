# Audio and MIDI I/O

- Own device and stream configuration here. Open, stop, and reconfigure streams off the callback.
- Preserve device IDs, channel pairs, and timestamp semantics. Pass only preallocated,
  bounded work to the device callback.
