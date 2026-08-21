# flashctrl-sdk — Clean-Room Charter & Rules of Engagement

**License target:** Apache-2.0 (permissive). This SDK must remain freely usable by
proprietary AND open clients. Every rule below exists to keep that license valid.

**The one-line rule:** *We implement from the wire and from official documentation —
never from anyone's source code.*

All four target vendors are copyleft (PiKVM/NanoKVM GPLv3, JetKVM GPL-2.0, GL.iNet TBD).
Copying, translating, or deriving from their source would force copyleft onto this SDK and
break the Apache scheme. We therefore reverse-engineer the observable interface only.

---

## ✅ PERMITTED sources (green)

- **Live network observation** from real devices we control: HTTP requests/responses,
  WebSocket frames, WebRTC signaling (SDP/ICE), DataChannel messages, MJPEG/RTP streams.
- **Official vendor documentation**: API docs, wikis, user guides, vendor-published
  OpenAPI/JSON schemas presented as documentation.
- **Black-box experimentation**: send inputs, record outputs, infer behavior.
- **Public standards & RFCs**: USB HID, WebRTC, ICE/STUN/TURN, DTLS-SRTP, RTP, MJPEG, JWT.
- **Our own captures, notes, and code** derived solely from the above.
- **Interface facts** — endpoint paths, port numbers, JSON field names, status codes —
  when recorded *from wire observation*. These are functional facts, not copyrightable
  expression, and every one in our capability matrix came off the wire.

## ❌ FORBIDDEN sources (red)

- Copying, pasting, or paraphrasing **source code** from any GPL/copyleft project
  (kvmd, NanoKVM, JetKVM, GL.iNet firmware) or **any third-party SDK**.
- **Reading their source to understand an implementation** — this muddies clean-room
  provenance even without copy/paste. Prefer the wire.
- **Translating** copyleft code between languages (still a derivative work).
- Lifting **protocol constants, magic values, or lookup tables** out of source —
  observe them on the wire instead.
- Incorporating any **dependency** whose license is incompatible with outbound Apache-2.0
  (GPL/LGPL runtime deps). Permissive only: MIT, BSD, ISC, Apache-2.0, MPL-2.0.
- **Vendor trademarks** in package identity or branding (nominative use only:
  "works with PiKVM" is fine; naming a module `pikvm` as branding is not).

## ⚠️ GRAY — handle with care

- Vendor docs that embed copyleft code snippets → cite the *wire observation*, not the snippet.
- Community reverse-engineering write-ups → check their provenance before relying on them;
  a blog that copied GPL source is not a clean source.
- If a fact is *only* obtainable from source and never appears on the wire → document why,
  and prefer to derive it experimentally.

---

## Provenance discipline (per adapter)

Each `adapters/<vendor>/` carries a `PROVENANCE.md` recording:
- Wire captures used (HAR / WS logs / pcap references), with dates and firmware versions.
- Documentation URLs consulted.
- An explicit statement: **"No source code from any third-party project was read or copied."**

Commit messages reference observations ("observed login POST returns JWT in cookie"),
never source ("ported from kvmd auth.py" — forbidden).

Keep raw capture evidence in `docs/captures/` as the audit trail.

## Dependency & CI hygiene

- Runtime dependencies: permissive licenses only (allowlist enforced in CI).
- WebRTC: use a BSD-licensed stack (libwebrtc / pion-derived bindings), never a GPL one.
- A license-scan step fails the build on any GPL/LGPL runtime dependency.

## If in doubt

Stop and ask. A single contaminated file can invalidate the license posture for the whole
SDK. The wire is always the safe path; the source is never worth the risk.
