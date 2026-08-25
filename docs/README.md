# flashDK documentation

## The project's own record

| | |
|---|---|
| [STATE.md](STATE.md) | What is true right now, per vendor, including what's unverified |
| [decisions.md](decisions.md) | Numbered decisions, with cost and rejected alternatives |
| [architecture.md](architecture.md) | How the core/adapter split works and why |
| [security.md](security.md) | Trust boundaries, credential handling, per-vendor transport security |
| [credits.md](credits.md) | Dependencies, licences, and what was evaluated and rejected |
| [ROADMAP.md](ROADMAP.md) | Planned vendor and device-class expansion |
| [TEST-MATRIX.md](TEST-MATRIX.md) | Configuration dimensions to exercise per platform |

## Evidence

`captures/` holds the raw wire-capture notes behind each adapter: what was sent,
what came back, and how a protocol detail was confirmed. This is the audit trail
[CLEANROOM.md](../CLEANROOM.md) requires.

## Reading order

New to the project: [README.md](../README.md), then [STATE.md](STATE.md) for the
current picture, then [architecture.md](architecture.md) for how it's put together.

Deciding whether to build on flashDK: read [decisions.md](decisions.md) and
[security.md](security.md) first. Together they show what was traded away and what
the trust model actually is, which is faster than reading the adapters themselves.

Adding a vendor: [CLEANROOM.md](../CLEANROOM.md) and
[CONTRIBUTING.md](../CONTRIBUTING.md) first, non-negotiably. Then a `captures/`
entry for the new vendor before any code.
