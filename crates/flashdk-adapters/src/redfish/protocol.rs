//! Pure request/response shapes for the DMTF Redfish standard: no HTTP I/O here, so
//! every mapping below is unit-tested against literal values taken directly from the
//! official DSP8010 JSON Schema bundle, the same discipline `nut::protocol` uses
//! against RFC 9271. See `PROVENANCE.md` for exactly which fact came from which
//! schema file.

use flashdk_core::power::PowerAction;
use serde::Deserialize;

/// The `ResetType` enum from `Resource.json` (referenced by
/// `ComputerSystem.Reset`'s `ResetType` parameter). Only the values this adapter
/// actually issues are represented; the full enum has more (`Nmi`, `Pause`,
/// `Sleep`, ...) that no [`PowerAction`] variant maps onto today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetType {
    On,
    ForceOff,
    ForceRestart,
    PushPowerButton,
}

impl ResetType {
    /// The exact string the schema enum uses on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            ResetType::On => "On",
            ResetType::ForceOff => "ForceOff",
            ResetType::ForceRestart => "ForceRestart",
            ResetType::PushPowerButton => "PushPowerButton",
        }
    }
}

/// Map flashDK's vendor-neutral [`PowerAction`] onto a Redfish `ResetType`.
///
/// `ShortPress` -> `PushPowerButton` and `LongPress` -> `ForceOff` come straight
/// from `ComputerSystem.Reset`'s own long description in the official schema:
/// "the `PushPowerButton` value shall perform or emulate an ACPI Power Button
/// Push, and the `ForceOff` value shall perform an ACPI Power Button Override,
/// commonly known as a four-second hold of the power button." `Reset` maps to
/// `ForceRestart` (an immediate, non-graceful restart) since flashDK's `Reset`
/// models a hardware reset line, not a graceful OS-initiated restart.
pub fn reset_type_for(action: PowerAction) -> ResetType {
    match action {
        PowerAction::On => ResetType::On,
        PowerAction::ShortPress => ResetType::PushPowerButton,
        PowerAction::LongPress => ResetType::ForceOff,
        PowerAction::Reset => ResetType::ForceRestart,
    }
}

/// Build the `POST` body for `#ComputerSystem.Reset`: `{"ResetType": "<value>"}`,
/// per `ComputerSystem.v1_28_0.json`'s `Reset` action definition.
pub fn reset_body(reset_type: ResetType) -> serde_json::Value {
    serde_json::json!({ "ResetType": reset_type.as_str() })
}

/// Build the session-login body: `{"UserName": ..., "Password": ...}`. The field
/// names are confirmed directly against `Session.v1_7_0.json`'s own `UserName` and
/// `Password` properties; the login handshake itself (POST to `Sessions`, token
/// returned via `X-Auth-Token`) is documented in DSP0266, not a schema file. See
/// `PROVENANCE.md`.
pub fn login_body(username: &str, password: &str) -> serde_json::Value {
    serde_json::json!({ "UserName": username, "Password": password })
}

/// Build the `POST` body for `#VirtualMedia.InsertMedia`: `Image` is the only
/// required parameter (`VirtualMedia.v1_6_5.json`); `Inserted` defaults to `true`
/// per the schema if omitted, so this adapter omits it rather than repeating the
/// default.
pub fn insert_media_body(image_uri: &str) -> serde_json::Value {
    serde_json::json!({ "Image": image_uri })
}

/// The header name the login response carries the session token in (DSP0266).
pub const AUTH_TOKEN_HEADER: &str = "X-Auth-Token";

/// Just enough of a `ComputerSystem` resource to read `PowerState` from
/// (`ComputerSystem.v1_28_0.json`). Other fields are ignored via `serde(default)`
/// so this stays forward-compatible with newer schema versions.
#[derive(Debug, Deserialize)]
pub struct SystemPowerState {
    #[serde(default)]
    pub power_state: Option<String>,
}

/// Redfish's `PowerState` enum values that map to "the machine is on" versus
/// "off", per the resource's own description. Transitional states
/// (`PoweringOn`/`PoweringOff`) are treated as `None`: honestly unknown rather
/// than guessed, matching how `PowerState.powered` is documented in
/// `flashdk_core::power` as `Option<bool>` for signals a device can't cleanly
/// answer.
pub fn power_state_to_bool(power_state: &str) -> Option<bool> {
    match power_state {
        "On" => Some(true),
        "Off" => Some(false),
        _ => None,
    }
}

/// Just enough of a `VirtualMedia` resource (`VirtualMedia.v1_6_5.json`) to build
/// a [`flashdk_core::media::MediaImage`] from it.
#[derive(Debug, Deserialize)]
pub struct VirtualMediaResource {
    #[serde(rename = "Id", default)]
    pub id: String,
    #[serde(rename = "Image", default)]
    pub image: Option<String>,
    #[serde(rename = "ImageName", default)]
    pub image_name: Option<String>,
    #[serde(rename = "Inserted", default)]
    pub inserted: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_type_strings_match_schema_enum() {
        assert_eq!(ResetType::On.as_str(), "On");
        assert_eq!(ResetType::ForceOff.as_str(), "ForceOff");
        assert_eq!(ResetType::ForceRestart.as_str(), "ForceRestart");
        assert_eq!(ResetType::PushPowerButton.as_str(), "PushPowerButton");
    }

    #[test]
    fn power_action_maps_per_schema_long_description() {
        assert_eq!(reset_type_for(PowerAction::On), ResetType::On);
        assert_eq!(
            reset_type_for(PowerAction::ShortPress),
            ResetType::PushPowerButton
        );
        assert_eq!(reset_type_for(PowerAction::LongPress), ResetType::ForceOff);
        assert_eq!(reset_type_for(PowerAction::Reset), ResetType::ForceRestart);
    }

    #[test]
    fn reset_body_shape() {
        assert_eq!(
            reset_body(ResetType::ForceRestart),
            serde_json::json!({ "ResetType": "ForceRestart" })
        );
    }

    #[test]
    fn login_body_shape() {
        assert_eq!(
            login_body("admin", "hunter2"),
            serde_json::json!({ "UserName": "admin", "Password": "hunter2" })
        );
    }

    #[test]
    fn insert_media_body_shape() {
        assert_eq!(
            insert_media_body("http://example.invalid/os.iso"),
            serde_json::json!({ "Image": "http://example.invalid/os.iso" })
        );
    }

    #[test]
    fn power_state_mapping() {
        assert_eq!(power_state_to_bool("On"), Some(true));
        assert_eq!(power_state_to_bool("Off"), Some(false));
        assert_eq!(power_state_to_bool("PoweringOn"), None);
    }

    #[test]
    fn virtual_media_resource_parses_schema_fields() {
        let json = serde_json::json!({
            "Id": "Cd1",
            "Image": "http://example.invalid/os.iso",
            "ImageName": "os.iso",
            "Inserted": true
        });
        let vm: VirtualMediaResource = serde_json::from_value(json).unwrap();
        assert_eq!(vm.id, "Cd1");
        assert_eq!(vm.image.as_deref(), Some("http://example.invalid/os.iso"));
        assert_eq!(vm.image_name.as_deref(), Some("os.iso"));
        assert_eq!(vm.inserted, Some(true));
    }
}
