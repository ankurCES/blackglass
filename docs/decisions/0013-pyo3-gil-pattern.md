# ADR 0013: GIL-acquire / spawn_blocking / GIL-release pattern for the Python bridge

- Status: Accepted (sub-plan 4)
- Context: the Python sidecar is loaded by pyo3 in the same process as the Tokio runtime. Naive `Python::with_gil` calls from async code block the runtime thread (the GIL plus Python evaluation can take milliseconds; impacket's network I/O can take seconds). We need a pattern that (a) lets the runtime make progress while a Python call is in flight, and (b) does not pin a Tokio worker thread on Python work.
- Decision: every pyo3 method on `RealBridge` follows the three-step pattern:
  1. `Python::with_gil(|py| { ... })` to marshal Rust args into a Python tuple/dict and acquire the target callable. This block is short — microseconds to a few milliseconds — and the GIL is released the moment the block returns.
  2. The blocking work (impacket exec, scapy serialization, requests call) runs inside `tokio::task::spawn_blocking(move || { ... })`. The closure takes the GIL again only long enough to call the Python function; the GIL is released for the duration of the I/O bound body because impacket's socket I/O releases it naturally.
  3. The `JoinHandle` is awaited from the async caller, the result is unmarshaled (with a final brief GIL acquire for object extraction), and the outcome is returned.
- Consequences:
  - One Tokio blocking-pool thread per concurrent Python call. The blocking pool defaults to 512 threads, so even hundreds of concurrent scapy/impacket calls do not starve the runtime.
  - The GIL is held only for actual Python C-API work (marshal in, call into pyo3, marshal out). The expensive I/O releases the GIL.
  - The pattern is uniform across all 6 sidecar capabilities (scapy, impacket, hardware), so a single test suite (`crates/python-bridge/tests/`) covers it.
  - We never call `Python::allow_threads` from inside a `with_gil` block — that would deadlock.
- Alternatives:
  - Spawn a dedicated OS thread per request (rejected: unbounded threads, no backpressure).
  - Run the sidecar as a separate process over stdin/stdout (rejected: doubles ops surface, breaks hermetic tests, doesn't work with `cargo test`).
  - Use `pyo3-asyncio` end-to-end (rejected: still requires a Tokio-compatible pyo3 runtime configuration; the spawn_blocking pattern is more portable across pyo3 versions).
