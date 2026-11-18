# Realtime Engine

- Follow the root callback contract for the mixer and its callees. Prepare buffers and
  reclaim retired state off the callback.
- Keep plugin RPC, lifecycle and teardown on worker or broker paths.
- Preserve event order, generation checks and latency alignment when changing scheduling.
