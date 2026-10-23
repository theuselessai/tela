# Monotonic elapsed time

`performance.now()` returns integer milliseconds since this JavaScript context's
clock was registered. It is monotonic and independent of `Date.now()` or wall-clock
adjustments. No permission is required.

Use differences between samples in the same context for operation durations or
request/reply RTT. Do not compare values across engines, processes or devices,
and do not interpret them as timestamps or keypress-to-render latency without
appropriate instrumentation. This is a small API subset, not browser Performance
Timeline support.
