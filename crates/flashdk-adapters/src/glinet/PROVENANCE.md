# Provenance: `flashdk_adapters::glinet`

Per [CLEANROOM.md](../../../../CLEANROOM.md), every wire-shape fact here traces to
live network observation against a real GL.iNet Comet (GL-RM1) this project owns,
captured directly with `curl` against `https://10.0.10.22/`. The device's own
served frontend JavaScript bundle was deliberately never opened or read: even
though it's served by a device we own, reading it to understand the API would be
"reading source to understand an implementation," which CLEANROOM.md forbids
regardless of copy/paste. Every endpoint, field name, and error shape below came
from an actual request/response pair, recorded in full in
[docs/captures/glinet-comet-kvmd-api.md](../../../../docs/captures/glinet-comet-kvmd-api.md).

## What the wire told us

`GET /api/info` reports daemon names (`kvmd-webrtc`, `kvmd-media`, `kvmd-vnc`,
`kvmd-webterm`, ...) and a `system.kvmd.version` field, establishing that this
device runs the `kvmd` daemon stack, the same family PiKVM's adapter targets.
This is a live-observed fact about GL.iNet's own device, not a claim borrowed
from PiKVM's documentation or source. Because of it, this module reuses two
things from `crate::pikvm`, both safe to share:

- `crate::pikvm::keymap`: a table mapping USB HID usage IDs to W3C
  `KeyboardEvent.code` strings. Both are public standards; the table is
  flashDK's own code derived from those specs, not vendor source, so sharing
  it between two `kvmd`-family adapters isn't a clean-room concern.
- The absolute-mouse coordinate transform (`core`'s `0..=32767` linearly
  mapped to kvmd's roughly `-32768..=32767` per axis), **not independently
  re-derived against this specific device** (no host was attached to observe
  a cursor actually land somewhere), but reused on the documented assumption
  that both devices' `kvmd`-family backends share the same convention. Flagged
  here explicitly rather than silently inherited.

## What's independently verified for this device

Every endpoint below was actually called against the real Comet unit and
returned the response shown in the capture doc: `/api/auth/login` (login),
`/api/auth/logout`, `/api/info`, `/api/hid`, `/api/hid/events/send_key`,
`/api/hid/events/send_mouse_move`, `/api/hid/events/send_mouse_button`,
`/api/hid/events/send_mouse_wheel`, `/api/atx`, `/api/atx/click` (`power`,
`power_long`, and `reset` buttons all confirmed), and `/api/msd` (read-only;
no image was available to test mount/connect against).

## Auth flow, the one real difference from PiKVM

`POST /api/auth/login` (form-encoded `user`/`passwd`, not JSON) returns
`{"ok": true, "result": {"token": "..."}}`; every later request carries that
token as a bare `Token: <token>` header (not `Authorization: Bearer`, which
was tried and rejected). This is the one meaningful protocol difference from
PiKVM's kvmd, which authenticates every request with static
`X-KVMD-User`/`X-KVMD-Passwd` headers and has no login/session concept at
all. That difference is why `GlInetKvm` is its own adapter with its own
constructor (`connect`, async, since it must log in) rather than a thin
wrapper around `PiKvm`.

## What isn't verified yet

No host was attached to the Comet's capture port during this session, so
HID/power actions are verified as accepted API calls (`ok: true`), not as
producing an observable effect on a real target machine. See
`docs/captures/glinet-comet-kvmd-api.md`'s "What this capture does not
establish" section and `docs/STATE.md` for the exact confidence tier.
