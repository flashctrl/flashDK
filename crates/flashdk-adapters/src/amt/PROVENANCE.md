# Provenance: `flashdk_adapters::amt`

Per [CLEANROOM.md](../../../../CLEANROOM.md), every wire-shape fact here traces to
official, published standards documents: DMTF's WS-Management specification
(DSP0226), WS-Management CIM Binding Specification (DSP0227), and WS-CIM Mapping
Specification (DSP0230), all downloaded directly as PDFs from `dmtf.org` and read
in full, plus Intel's own AMT SDK documentation (see
`../redfish/../../../docs/ROADMAP.md`'s AMT section for that half of the sourcing,
already landed). This project has no AMT 3.0+ hardware to verify any of this
against live yet; see `docs/STATE.md`.

## What DSP0226/DSP0227/DSP0230 establish, verbatim

- **The SOAP envelope shape** (DSP0226 §5.4.2, the "default addressing model"):
  `wsa:To`, `wsman:ResourceURI` (with `s:mustUnderstand="true"`),
  `wsman:SelectorSet`/`wsman:Selector`, `wsa:Action`, `wsa:MessageID`,
  `wsa:ReplyTo` (the well-known anonymous URI,
  `http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous`, for a
  same-connection reply), and an optional `wsman:OperationTimeout` (an
  `xs:duration`, e.g. `PT30S`) as SOAP headers, with the operation's actual
  payload in `s:Body`. Quoted directly from DSP0226's own worked examples.
- **How a CIM method call ("extrinsic method") maps onto that envelope**
  (DSP0227 clause 11, DSP0230 clause 9.5 and 10.3):
  - The request body element is named `<MethodName>_INPUT`, a complex type
    containing one child element per `IN` parameter, in the CIM method's own
    parameter order. The response body element is `<MethodName>_OUTPUT`,
    containing the `OUT` parameters plus a final `ReturnValue` element. Both
    are quoted directly from DSP0230's own worked example (§9.5.2).
  - The request `wsa:Action` is `<class-namespace>/<MethodName>`; the response
    `wsa:Action` is `<class-namespace>/<MethodName>Response`. Quoted directly
    from DSP0230 §10.3's own rule and example.
  - The `wsman:ResourceURI` for a standard CIM class is "identical to the XML
    namespace URI of the schema for the class" (DSP0227 §6.1, quoted). For a
    class with keyed instances, `wsman:SelectorSet`/`wsman:Selector` carries
    the key values, with the key's own CIM name as the `Name` attribute.

## Applying this to `CIM_PowerManagementService.RequestPowerStateChange`

`RequestPowerStateChange`'s parameters (`PowerState`, `ManagedElement`,
`Time`, `Job`, `TimeoutPeriod`) and its `PowerState` value enum are already
sourced from Intel's own AMT SDK documentation; see
`docs/ROADMAP.md`'s "Intel vPro AMT" section. Combining that with the general
DSP0230 mapping rule above gives the request shape this module builds:

- `wsa:Action`: `<CIM_PowerManagementService's namespace>/RequestPowerStateChange`
- `wsman:ResourceURI`: the same namespace, per DSP0227 §6.1's rule that a
  standard CIM class's ResourceURI equals its schema namespace.
- Body: `<p:RequestPowerStateChange_INPUT xmlns:p="...">` containing
  `PowerState`, `ManagedElement` (a reference, per its `REF` type in Intel's
  own method signature), and `TimeoutPeriod` child elements, per DSP0230's
  general `_INPUT` structure rule.

**What's a documented inference, not yet a confirmed literal fact:** the
exact namespace string `CIM_PowerManagementService` resolves to
(`http://schemas.dmtf.org/wbem/wscim/1/cim-schema/2/CIM_PowerManagementService`,
following DSP0227 §6.1's general naming pattern and its own worked example
for `CIM_SoftwareElement`) has not been independently confirmed against a
literal string Intel's own documentation quotes, nor against a live AMT
unit's response. It is the DMTF-standard construction for a DMTF-defined CIM
class name, which `CIM_PowerManagementService` is, so this is a
well-grounded inference rather than a guess, but it is flagged here rather
than silently treated as verified.

## What this module does not attempt yet

- No HTTP client or Digest/Kerberos authentication handshake: that needs a
  live capture against a provisioned AMT 3.0+ unit, which this project
  doesn't have yet (its one real vPro unit reports AMT 1.2, which predates
  WS-Management; see `docs/ROADMAP.md`).
- No response-parsing for a real success/fault body: `RequestPowerStateChange`
  responses and DSP0227's `CIM_Error` fault shape are sourced (see above and
  DSP0227 §12.1's own worked fault example) but not yet exercised against a
  live message.
- KVM redirection (`IPS_KVMRedirectionSettingData`, RFB 4.0) is unrelated to
  WS-Management and isn't part of this module at all; see
  `docs/ROADMAP.md`.
