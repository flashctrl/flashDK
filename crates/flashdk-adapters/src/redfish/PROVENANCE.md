# Provenance: `flashdk_adapters::redfish`

Per [CLEANROOM.md](../../../../CLEANROOM.md), every wire-shape fact here traces to
one of two sources: the published DMTF Redfish specification (DSP0266) or the
official DMTF Redfish JSON Schema bundle (DSP8010). No BMC firmware, vendor SDK,
or vendor source code was read to build this adapter. This project does not own
an iDRAC or iLO unit, so none of this has been exercised against real hardware
yet; see the module doc comments and `docs/STATE.md` for what's confirmed versus
compiled-but-unverified.

## Primary sources fetched directly (raw JSON, not summarized)

All of the following were downloaded from `redfish.dmtf.org` and read as raw
JSON, the same discipline used for the NUT adapter's RFC text:

- `schemas/v1/ComputerSystem.v1_28_0.json`: the `#ComputerSystem.Reset` action
  shape (`{"ResetType": "<value>"}`) and the `PowerState` property.
- `schemas/v1/Resource.json`: the `ResetType` enum itself (`On`, `ForceOff`,
  `GracefulShutdown`, `GracefulRestart`, `ForceRestart`, `Nmi`, `ForceOn`,
  `PushPowerButton`, `PowerCycle`, `Suspend`, `Pause`, `Resume`,
  `FullPowerCycle`, `Sleep`, `Hibernate`), including its `enumDescriptions`.
- `schemas/v1/VirtualMedia.v1_6_5.json`: the `InsertMedia` action
  (`{"Image": "<uri>", ...}`, `Image` required) and `EjectMedia` action (no
  parameters), plus the `Image`, `ImageName`, and `Inserted` resource
  properties.
- `schemas/v1/SessionService.v1_2_0.json` and `schemas/v1/Session.v1_7_0.json`:
  confirm `UserName`/`Password` as the session resource's own properties
  (`Password` is `null` in responses, matching a write-only login field).
- `schemas/v1/ServiceRoot.v1_18_0.json`: confirms `Systems`, `Managers`, and
  `SessionService` as root-level links used for discovery, rather than
  hardcoding vendor-specific paths.

## Corroborated, not directly re-quoted from a downloaded schema file

The session login handshake itself, specifically that authenticating is a
`POST` to the `Sessions` collection under `SessionService` with a JSON body of
`{"UserName": "...", "Password": "..."}`, returning the session token in an
`X-Auth-Token` response header and the new session's URI in a `Location`
header, is documented in the core Redfish specification (DSP0266, session
login and `X-Auth-Token` sections), not the schema-definition files. The
`UserName`/`Password` field names and the `Password: null` in responses were
independently confirmed against `Session.v1_7_0.json` above, so this is
corroborated from two DMTF-published documents rather than a single quoted
excerpt. Treat the exact request/response header casing as high-confidence,
not verified against a live login yet.

## What this adapter does not attempt

The Redfish graphical console (iDRAC's and iLO's own HTML5 virtual console) is
proprietary per vendor, not part of the Redfish standard, and stays out of
scope here entirely, per `docs/ROADMAP.md`.
