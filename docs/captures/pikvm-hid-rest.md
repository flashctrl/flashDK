# Capture — PiKVM HID REST contract (wire-observed)

Device: PiKVM v3, kvmd 4.206, https://10.0.10.20 (self-signed TLS)
Auth: header `X-KVMD-User` / `X-KVMD-Passwd`
Method: POST. Params accepted as query string. Response: `{"ok":bool,"result":{...}}`.

Observed endpoints under /api/hid/events/ (GET returns 405 => endpoint exists):

| Endpoint            | Params                          | Notes |
|---------------------|---------------------------------|-------|
| send_key            | key=<KeyboardEvent.code>, state=<bool> | key rejected unless a valid W3C code name (e.g. "KeyA","Enter") |
| send_mouse_button   | button=<left|right|middle>, state=<bool> | |
| send_mouse_move     | to_x=<int>, to_y=<int>          | absolute; kvmd range approx [-32768,32767] |
| send_mouse_relative | delta_x=<int>, delta_y=<int>    | |
| send_mouse_wheel    | delta_x=<int>, delta_y=<int>    | |

Confirmed via safe no-ops (key release of unpressed key, zero-delta wheel/relative,
absolute move to 0,0). No third-party source consulted; contract inferred from live
responses + validator error messages only.
