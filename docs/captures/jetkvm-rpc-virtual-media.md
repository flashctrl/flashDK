# JetKVM `rpc` channel: virtual media, Wake-on-LAN, extensions

**Captured:** 2026-10-07, against an owned JetKVM at 10.0.10.21 (HTTP, logged in),
attached to a disposable Bazzite target. Method: the official web client was driven
in a browser while `RTCDataChannel.prototype.send` was wrapped to log every JSON-RPC
frame sent and received on the `rpc` channel. Only DataChannel message text was
observed; the device's served frontend bundle was not read.

Framing is JSON-RPC 2.0, one JSON object per DataChannel message. Responses to
success carry `result`, which is omitted entirely when the method returns nothing.

## Virtual media (observed, round trip completed)

| Direction | Frame |
|---|---|
| out | `{"method":"getVirtualMediaState","params":{},"id":8}` |
| in (nothing mounted) | `{"result":null,"id":8}` |
| out | `{"method":"listStorageFiles","params":{},"id":10}` |
| in | `{"result":{"files":[{"filename":"OPNsense-26.1.2-dvd-amd64.iso","size":2207027200,"createdAt":"2026-04-13T22:04:27.786719896Z"}]},"id":10}` |
| out | `{"method":"getStorageSpace","params":{},"id":11}` |
| in | `{"result":{"bytesUsed":2308558848,"bytesFree":11853598720},"id":11}` |
| out (Mount File, "CD/DVD") | `{"method":"mountWithStorage","params":{"filename":"OPNsense-26.1.2-dvd-amd64.iso","mode":"CDROM"},"id":12}` |
| in | `{"id":12}` (no `result` field) |
| in (state after mount) | `{"result":{"source":"Storage","mode":"CDROM","filename":"OPNsense-26.1.2-dvd-amd64.iso","size":2207027200},"id":13}` |
| out (Unmount) | `{"method":"unmountImage","params":{},"id":15}` |
| in | `{"id":15}` then `getVirtualMediaState` returns `{"result":null}` |

The device also pushes un-requested notifications with no `id`, for example
`{"method":"usbState","params":"not attached"}` followed by `"configured"` around a
mount or unmount (the USB gadget re-enumerates).

The UI offers a second source, "URL Mount" (labelled Experimental), and a "Disk"
mode alongside "CD/DVD". Neither was exercised. Upload of a new image is also not
captured.

## Wake-on-LAN (read only)

`getWakeOnLanDevices` with `{}` returned `{"result":[]}` (none configured). Sending
a magic packet was not captured.

## Extensions (read only; power is NOT captured)

`getActiveExtension` returned `{"result":""}`. The UI lists "ATX Power Control",
"DC Power Control" and "Serial Console", each behind a Load button. Loading one
changes device configuration, so it was not done. The method names and payloads for
loading an extension and for the ATX power actions remain uncaptured.

## Side effect worth knowing

Keystrokes sent in the browser while the video element has focus go to the target.
A single `Escape` opened the Steam sidebar on the Bazzite machine during this
capture.
