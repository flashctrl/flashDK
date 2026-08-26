# Capture: GL.iNet Comet (GL-RM1), live wire observation

**Date:** 2026-08-26
**Device:** GL.iNet Comet, freshly provisioned (admin password set, no other
config), no host attached to its capture port.
**Firmware/platform, as self-reported by `/api/info`:** `kvmd` 4.82, kernel
6.1.141 (`aarch64`), platform base "Rockchip RV1126B-P EVB V14 Board", board
`rpi4`, model `v3`.
**Method:** direct HTTPS requests (`curl -k`, ignoring the device's own
self-signed certificate, the same trust model PiKVM's TOFU pinning already
handles) against `https://10.0.10.22/`. No vendor source, firmware image, or
frontend JavaScript bundle was read; the frontend bundle actually served by
this device was deliberately left unopened, per [CLEANROOM.md](../../CLEANROOM.md)'s
"reading source to understand an implementation" prohibition. Every fact below
came from an actual request/response pair against the real device.

## Finding: this is a `kvmd` device

`GET /api/info` (once authenticated) reports `system.kvmd.version` and a set
of `extras` daemons named `kvmd-ipmi`, `kvmd-webrtc`, `kvmd-media`,
`kvmd-vnc`, `kvmd-webterm`, plus a `streamer.app` of `ustreamer`. GL.iNet's
Comet firmware is running the `kvmd` daemon stack, the same family PiKVM
runs, confirmed by the device's own live API response rather than by reading
any source. This does not mean the two are identical (see the auth
difference below), and every endpoint used by the `glinet` adapter is
independently verified against this device rather than assumed from PiKVM's
behavior.

## Auth: login-and-token, not PiKVM's static per-request headers

Unlike PiKVM's kvmd, which authenticates every request with static
`X-KVMD-User`/`X-KVMD-Passwd` headers, this device uses a login exchange:

```
POST /api/auth/login
Content-Type: application/x-www-form-urlencoded

user=admin&passwd=<password>
```

Response:

```json
{"ok": true, "result": {"token": "<64-char hex string>"}}
```

A JSON body (`{"user": ..., "passwd": ...}`) is rejected with a
`ValidatorError`; the endpoint wants form-encoded fields. The response also
sets an `HttpOnly` `auth_token` cookie (`SameSite=Strict`), a different value
from the `token` field, presumably for the browser SPA's own session use;
this adapter uses the JSON `token` value, not the cookie.

Every subsequent request is authenticated with:

```
Token: <token>
```

(a bare custom header, not `Authorization: Bearer ...`, which was tried and
rejected with `UnauthorizedError`). `POST /api/auth/logout` with the same
header invalidates the session (`{"ok": true, "result": {}}`).

## Verified endpoints (all live, `ok: true` responses)

| Endpoint | Method | Params | Notes |
|---|---|---|---|
| `/api/info` | GET | none | System/daemon identity (see above) |
| `/api/hid` | GET | none | HID subsystem status |
| `/api/hid/events/send_key` | POST | `key`, `state` | `key` is a W3C `KeyboardEvent.code` string (tested `KeyA`) |
| `/api/hid/events/send_mouse_move` | POST | `to_x`, `to_y` | Absolute move |
| `/api/hid/events/send_mouse_button` | POST | `button`, `state` | Tested `left` |
| `/api/hid/events/send_mouse_wheel` | POST | `delta_x`, `delta_y` | |
| `/api/atx` | GET | none | `{"busy", "enabled", "leds": {"hdd", "power"}, "power": "on"/"off"}` |
| `/api/atx/click` | POST | `button` | Tested `power`, `power_long`, `reset`, all accepted |
| `/api/msd` | GET | none | Virtual-media status; `storage.images` empty on this fresh unit |

`/api/atx/click?button=reset` returned `AtxIsBusyError` immediately after
`power_long` (the ATX subsystem was still finishing the prior action) and
succeeded cleanly a few seconds later; that's a real, meaningful error shape
this adapter should map, not a sign the endpoint doesn't exist.

## Follow-up capture: mouse output mode and virtual-media write

A second session against the same unit (still no host attached, but
`GET /api/hid` had started reporting `mouse.online`/`keyboard.online` as
`true` where the first capture saw `false`, suggesting a host may since have
been connected) confirmed two more things:

- `POST /api/hid/set_params?mouse_output=usb_rel` (and back to `usb`) really
  switches the device's single physical mouse HID endpoint between absolute
  and relative mode: `GET /api/hid` afterward shows `mouse.outputs.active`
  and `mouse.absolute` flip accordingly. `/api/hid/events/send_mouse_relative`
  (`delta_x`, `delta_y`) then returns `ok: true`. The two modes are mutually
  exclusive on this hardware, not simultaneously available, confirmed by this
  toggle rather than assumed.
- `POST /api/msd/write?image=<name>` (raw body = file bytes) and
  `POST /api/msd/remove?image=<name>` both work as expected for uploading and
  deleting a virtual-media image. `POST /api/msd/set_params?image=<name>`
  (selecting an image) also succeeds. `POST /api/msd/set_connected?connected=1`
  (actually presenting it to the host), tried against a trivial 5-byte test
  file, returned a plain-text `500 Internal Server Error`, not the device's
  usual JSON error shape: a real backend failure, most likely because a
  degenerate file isn't something the mass-storage gadget can actually back,
  not a sign the endpoint or request shape is wrong. Mount-with-a-real-image
  and the resulting observable "connected" transition remain unverified.

## What this capture does not establish

No host was attached to the capture port during this session, so none of the
HID/power actions above have an observable downstream effect to confirm
against (the target machine's own reaction to a keypress, a mouse cursor
actually moving, or a real ATX line firing). What's verified is the request
contract: the device's own API accepts these calls and returns success. The
absolute-mouse coordinate range (kvmd's usual roughly -32768..=32767 per
axis, per PiKVM's already-verified transform) was not independently
re-derived here; the `glinet` adapter reuses PiKVM's exact transform on the
assumption the two share the same `kvmd`-family coordinate convention, which
is a documented assumption, not an independently confirmed fact for this
specific device. `/api/msd/set_connected` (the actual mount step) has only
been exercised against a degenerate test file and failed server-side (see
above); a real image and an observable "host sees the drive" check remain
outstanding.
