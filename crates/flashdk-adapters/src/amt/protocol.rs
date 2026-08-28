//! Pure WS-Management SOAP envelope construction for AMT's
//! `CIM_PowerManagementService.RequestPowerStateChange`. No HTTP or auth here (see
//! the module doc comment and `PROVENANCE.md` for what's still missing before this
//! can talk to a real device); everything below is built and unit-tested against
//! literal rules and examples quoted from three official DMTF specifications:
//! DSP0226 (WS-Management), DSP0227 (WS-Management CIM Binding), and DSP0230
//! (WS-CIM Mapping). See `PROVENANCE.md` for exactly which fact traces to which
//! document and clause.

use flashdk_core::power::PowerAction;

/// The XML namespace DSP0227 §6.1's rule ("a standard CIM class's ResourceURI is
/// identical to the XML namespace URI of its schema") resolves to for
/// `CIM_PowerManagementService`, following the pattern DSP0227's own worked
/// example uses for `CIM_SoftwareElement`
/// (`http://schemas.dmtf.org/wbem/wscim/1/cim-schema/2/<ClassName>`). This is a
/// well-grounded inference from the documented naming rule, not yet an
/// independently confirmed literal string; see `PROVENANCE.md`.
pub const POWER_MANAGEMENT_SERVICE_NAMESPACE: &str =
    "http://schemas.dmtf.org/wbem/wscim/1/cim-schema/2/CIM_PowerManagementService";

/// DSP0230 §10.3's rule: a request action URI is `<class-namespace>/<MethodName>`.
pub fn request_action() -> String {
    format!("{POWER_MANAGEMENT_SERVICE_NAMESPACE}/RequestPowerStateChange")
}

/// DSP0230 §10.3's rule: a response action URI is `<class-namespace>/<MethodName>Response`.
pub fn response_action() -> String {
    format!("{POWER_MANAGEMENT_SERVICE_NAMESPACE}/RequestPowerStateChangeResponse")
}

/// DSP0226's well-known anonymous reply-to URI, used when the reply is expected on
/// the same connection as the request (quoted directly from DSP0226's own worked
/// examples).
pub const ANONYMOUS_REPLY_TO: &str =
    "http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous";

/// `CIM_PowerManagementService.RequestPowerStateChange`'s `PowerState` parameter
/// values, per Intel's own AMT SDK documentation (see `docs/ROADMAP.md`'s "Intel
/// vPro AMT" section). Only the four values Intel's docs call "always supported"
/// are represented; the rest (`Sleep`, `Hibernate`, the "Graceful" variants, ...)
/// are conditional on OS integration and not mapped here yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    PowerUp,
    PowerCycle,
    PowerDown,
    MasterBusReset,
}

impl PowerState {
    /// The numeric value AMT's `PowerState` parameter expects on the wire.
    pub fn as_u16(self) -> u16 {
        match self {
            PowerState::PowerUp => 2,
            PowerState::PowerCycle => 5,
            PowerState::PowerDown => 8,
            PowerState::MasterBusReset => 10,
        }
    }
}

/// Map flashDK's vendor-neutral [`PowerAction`] onto an AMT `PowerState`.
///
/// `On` -> `PowerUp` and `Reset` -> `MasterBusReset` are direct matches. AMT has no
/// ACPI-style "soft push" concept the way Redfish's `PushPowerButton`/`ForceOff`
/// pair does; among the four always-supported values, `PowerDown` (an immediate,
/// non-graceful power-off) is the closest match for both `ShortPress` and
/// `LongPress`, so both map there for now. A graceful, OS-cooperating shutdown
/// (`PowerState` value 12, "Power Off-Soft Graceful") exists in AMT's full enum but
/// is conditional on OS integration, not always supported, so it isn't used here;
/// see `PROVENANCE.md`.
pub fn power_state_for(action: PowerAction) -> PowerState {
    match action {
        PowerAction::On => PowerState::PowerUp,
        PowerAction::ShortPress => PowerState::PowerDown,
        PowerAction::LongPress => PowerState::PowerDown,
        PowerAction::Reset => PowerState::MasterBusReset,
    }
}

/// Build the full SOAP envelope for a `RequestPowerStateChange` request.
///
/// `to` is the AMT device's WS-Management endpoint (e.g.
/// `"http://10.0.1.6:16992/wsman"`), `managed_element_selector` is the
/// `wsman:Selector` value identifying the target `CIM_ComputerSystem` instance
/// (device-specific; not sourced yet, see `PROVENANCE.md`), and `message_id` is a
/// caller-supplied UUID string for `wsa:MessageID` (DSP0226 requires a fresh,
/// unique ID per request; generation is the caller's responsibility so this
/// function stays pure and deterministic for testing).
///
/// Structure (headers, then body) follows DSP0226 §5.4.2's default addressing
/// model and DSP0230 §9.5's `_INPUT` message shape, both quoted in
/// `PROVENANCE.md`.
pub fn request_power_state_change_envelope(
    to: &str,
    managed_element_selector: &str,
    power_state: PowerState,
    message_id: &str,
) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:wsman="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd" xmlns:p="{ns}">
  <s:Header>
    <wsa:To>{to}</wsa:To>
    <wsman:ResourceURI s:mustUnderstand="true">{ns}</wsman:ResourceURI>
    <wsa:ReplyTo>
      <wsa:Address>{anon}</wsa:Address>
    </wsa:ReplyTo>
    <wsa:Action s:mustUnderstand="true">{action}</wsa:Action>
    <wsa:MessageID>{message_id}</wsa:MessageID>
    <wsman:SelectorSet>
      <wsman:Selector Name="Name">{selector}</wsman:Selector>
    </wsman:SelectorSet>
    <wsman:OperationTimeout>PT30S</wsman:OperationTimeout>
  </s:Header>
  <s:Body>
    <p:RequestPowerStateChange_INPUT>
      <p:PowerState>{power_state}</p:PowerState>
      <p:ManagedElement>{selector}</p:ManagedElement>
    </p:RequestPowerStateChange_INPUT>
  </s:Body>
</s:Envelope>"#,
        ns = POWER_MANAGEMENT_SERVICE_NAMESPACE,
        to = to,
        anon = ANONYMOUS_REPLY_TO,
        action = request_action(),
        message_id = message_id,
        selector = managed_element_selector,
        power_state = power_state.as_u16(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_state_values_match_amt_docs() {
        assert_eq!(PowerState::PowerUp.as_u16(), 2);
        assert_eq!(PowerState::PowerCycle.as_u16(), 5);
        assert_eq!(PowerState::PowerDown.as_u16(), 8);
        assert_eq!(PowerState::MasterBusReset.as_u16(), 10);
    }

    #[test]
    fn power_action_mapping() {
        assert_eq!(power_state_for(PowerAction::On), PowerState::PowerUp);
        assert_eq!(
            power_state_for(PowerAction::ShortPress),
            PowerState::PowerDown
        );
        assert_eq!(
            power_state_for(PowerAction::LongPress),
            PowerState::PowerDown
        );
        assert_eq!(
            power_state_for(PowerAction::Reset),
            PowerState::MasterBusReset
        );
    }

    /// DSP0230 §10.3: request action URI is `<class-namespace>/<MethodName>`.
    #[test]
    fn action_uris_follow_dsp0230_rule() {
        assert_eq!(
            request_action(),
            "http://schemas.dmtf.org/wbem/wscim/1/cim-schema/2/CIM_PowerManagementService/RequestPowerStateChange"
        );
        assert_eq!(
            response_action(),
            "http://schemas.dmtf.org/wbem/wscim/1/cim-schema/2/CIM_PowerManagementService/RequestPowerStateChangeResponse"
        );
    }

    /// Checks the envelope contains every header DSP0226 §5.4.2's default
    /// addressing model requires, and the `_INPUT` body shape DSP0230 §9.5
    /// defines, without depending on exact whitespace formatting.
    #[test]
    fn envelope_contains_required_headers_and_body_shape() {
        let xml = request_power_state_change_envelope(
            "http://10.0.1.6:16992/wsman",
            "ManagedSystem",
            PowerState::MasterBusReset,
            "uuid:test-message-id",
        );
        assert!(xml.contains("<wsa:To>http://10.0.1.6:16992/wsman</wsa:To>"));
        assert!(xml.contains(&format!(
            "<wsman:ResourceURI s:mustUnderstand=\"true\">{POWER_MANAGEMENT_SERVICE_NAMESPACE}</wsman:ResourceURI>"
        )));
        assert!(xml.contains(&format!(
            "<wsa:Action s:mustUnderstand=\"true\">{}</wsa:Action>",
            request_action()
        )));
        assert!(xml.contains("<wsa:MessageID>uuid:test-message-id</wsa:MessageID>"));
        assert!(xml.contains(ANONYMOUS_REPLY_TO));
        assert!(xml.contains("<wsman:Selector Name=\"Name\">ManagedSystem</wsman:Selector>"));
        assert!(xml.contains("<p:RequestPowerStateChange_INPUT>"));
        assert!(xml.contains("<p:PowerState>10</p:PowerState>"));
        assert!(xml.contains("</p:RequestPowerStateChange_INPUT>"));
    }
}
