<!--
SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->

# `nv-redfish-scraper-cli`

A comprehensive command-line tool built on top of `nv-redfish-scraper` and
`nv-redfish`. It walks a live BMC's Redfish tree, streams sensor readings,
and captures `ReconstructionRecord` snapshots that can be replayed later by
applications that own a scraper runtime.

The CLI is an example: it ships in `examples/redfish-scraper-cli/` and is
not published to crates.io. It is the canonical reference for how to wire
the `nv-redfish-scraper` runtime against an `HttpBmc` transport.

## Build

```bash
cargo build -p redfish-scraper-cli
```

The binary is produced at `target/debug/nv-redfish-scraper-cli` (or
`target/release/nv-redfish-scraper-cli` for `--release` builds).

## Connection flags

Every subcommand shares the same connection-flags block:

| Flag             | Default | Notes                                                  |
|------------------|---------|--------------------------------------------------------|
| `--bmc URL`      | _required_ | Redfish endpoint, e.g. `https://192.0.2.10/`.        |
| `--username U`   | _none_  | Pairs with `--password` for HTTP basic auth.            |
| `--password P`   | _none_  | Required when `--username` is set.                      |
| `--insecure`     | `false` | Accept self-signed TLS certificates.                    |
| `--bmc-id NAME`  | URL host | Tag carried by every emitted resource event.           |
| `--timeout DUR`  | `30s`   | HTTP request timeout. Suffix: `ms`, `s`, `m`, `h`.      |
| `--format MODE`  | `pretty`| `pretty` for humans, `jsonl` for piping into `jq`.      |
| `--verbose`      | `false` | Include `RuntimeOutput::Runtime(_)` events.             |
| `--stats`        | `false` | Print final `RuntimeStats` to stderr.                   |

## Quickstart

### 1. Discover the full tree

```bash
nv-redfish-scraper-cli \
    --bmc https://192.0.2.10/ \
    --username admin --password REDACTED --insecure \
    discover
```

Walk service-root → chassis (× N) → sensors → computer systems and print
every observed resource event. Add `--no-chassis`, `--no-sensors`, or
`--no-systems` to skip a layer; pass `--max-in-flight 8` to cap concurrent
fetches against the BMC.

### 2. Stream sensor readings on an interval

```bash
nv-redfish-scraper-cli \
    --bmc https://192.0.2.10/ --insecure \
    sensors --interval 5s
```

Walks the chassis tree once via the typed `nv-redfish` API and then enters
a tokio-interval loop that fetches every sensor link in parallel through
`EntityLink::fetch`. Pass `--once` to run a single read pass and exit, or
`--interval 250ms` for sub-second polling.

### 3. Capture a reconstruction snapshot

```bash
nv-redfish-scraper-cli \
    --bmc https://192.0.2.10/ --insecure \
    snapshot --output snapshot.jsonl
```

Runs the same discovery pass as `discover`, accumulates every
`RedfishResourceEvent`, derives `ReconstructionRecord`s via
`reconstruction_iter`, and writes one JSON object per line to the supplied
path. Use `--output -` to send JSONL to stdout, `--append` to extend an
existing file instead of truncating.

## Output formats

* `--format pretty` (default) — human-readable, one event per line. Looks
  like:

  ```text
  [inserted] ChassisCollection /redfish/v1/Chassis parent=/redfish/v1
  [inserted] Chassis /redfish/v1/Chassis/System.Embedded.1
  [inserted] Sensor /redfish/v1/Chassis/System.Embedded.1/Sensors/InletTemp parent=...
  ```

* `--format jsonl` — one JSON object per line, ready for `jq`:

  ```json
  {"kind":"resource","generator":"generator:1/0","latency_ms":42,"event":{"bmc_id":"...","odata_id":"...","change":"Inserted","payload":{"kind":"Chassis","odata_id":"...","etag":null},...}}
  ```

  `RuntimeOutput::Runtime(_)` events only appear under `--verbose`.

## TLS notes

BMC management ports usually present self-signed certificates rooted in the
BMC itself. Pass `--insecure` to accept them. For production deployments
that pin a CA, configure the rustls trust store at the system level rather
than relying on `--insecure`.

## v2 backlog (deferred)

These belong to follow-up phases and are intentionally not part of the v1
CLI:

* `watch` subcommand — long-running scrape loop with periodic re-fetch and
  change-classified resource events.
* `replay` subcommand — read a snapshot JSONL file and rebuild a runtime
  generator tree via `replay_records`. Replay needs the application to
  reconstruct typed generator handles per record (chassis / sensor /
  system); we will design the policy closure surface in a follow-up.
* Value-aware sensors generator — a scraper-side generator that emits a
  new `RedfishEvent::Telemetry(SensorReadingEvent)` variant. This is the
  right way to apply per-target throttling and cost-aware admission to
  telemetry. Today the CLI emits readings from a post-discovery tokio loop
  that bypasses the scraper.
