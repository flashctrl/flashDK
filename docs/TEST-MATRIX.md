# Platform Test Matrix (future coverage)

Each vendor adapter must be exercised across the configuration dimensions below, not
just the happy path. Many of these change the wire protocol or capability set, so they
need real-device verification per platform (NanoKVM, PiKVM, JetKVM, GL.iNet).

Status legend: ✅ verified · 🔶 partial/observed · ⬜ not yet tested · — n/a

| Dimension | Why it matters | NanoKVM | PiKVM | JetKVM | GL.iNet |
|-----------|----------------|---------|-------|--------|---------|
| **EDID changes** | Alters resolutions the target sees; adapters may expose get/set EDID | ⬜ | ⬜ | 🔶 (setEDID seen) | ⬜ |
| **HID class changes** | kbd/mouse/mass-storage/audio composition changes USB gadget + channels | ⬜ | ⬜ | 🔶 (composite gadget observed) | ⬜ |
| **Video codec** | WebRTC/H.264 vs MJPEG availability & fallback per device | 🔶 | 🔶 | 🔶 (WebRTC/H.264) | ⬜ |
| **Mouse mode: absolute vs relative** | Different frame types; some devices need mode toggle | 🔶 (abs only) | ✅ (both) | 🔶 (abs verified) | ⬜ |
| **Waking the machine** | Each platform wakes the target differently (HID nudge, WoL, power) | ⬜ | ⬜ | 🔶 ("Try to wake") | ⬜ |
| **Reset HID / HDMI** | Recovery from stuck input or lost video; per-platform method | ⬜ | ⬜ | ⬜ | ⬜ |
| **Virtual media** | Mount ISO/image; upload vs URL; CD-ROM vs disk | 🔶 | ✅ | ⬜ | ⬜ |
| **Power** | ATX/GPIO/extension; on/off/reset/long-press | ✅ | 🔶 (no ATX HW) | ⬜ (ext board) | ⬜ (ext board) |
| **Virtual keyboard** | On-screen keyboard path; may differ from raw HID | ⬜ | ⬜ | ⬜ | ⬜ |
| **Hotkeys / macros** | Chorded keys, leader sequences, saved macros | ⬜ | ⬜ | ⬜ | ⬜ |
| **Text pasting** | Bulk text entry; layout-dependent keycode translation | ⬜ | ⬜ | ⬜ | ⬜ |

## Notes
- **Absolute vs relative mouse** and **EDID/HID-class** changes are the highest-value
  matrix cells: they most often shift the wire protocol, so each combination wants a
  capture, not an assumption.
- **Text pasting** is layout-dependent (usage-code translation); test multiple layouts.
- **Video codec** and **wake/reset** paths should be probed per firmware version — these
  vendors ship breaking changes (esp. JetKVM, NanoKVM).
- Fill cells only from real-device verification (clean-room), never from vendor source.
