//! The NUT (Network UPS Tools) network protocol: a plain-text, line-oriented TCP
//! protocol standardized as IETF RFC 9271. Every command and response shape here is
//! taken directly from the RFC text (quoted inline where it matters) or from NUT's
//! own official manual pages; see `PROVENANCE.md` in this directory for exactly
//! which fact came from where. No NUT source code was read.
//!
//! Encoding and parsing are kept pure and synchronous here, free of the TCP I/O in
//! `mod.rs`, so they can be unit-tested against literal protocol text without a
//! running `upsd` to connect to.

use flashdk_core::ups::PowerSource;
use flashdk_core::{Error, Result};

/// Build the `USERNAME <name>` command line (RFC 9271 §4.2, sent before `PASSWORD`).
pub fn cmd_username(name: &str) -> String {
    format!("USERNAME {name}\n")
}

/// Build the `PASSWORD <pass>` command line.
pub fn cmd_password(pass: &str) -> String {
    format!("PASSWORD {pass}\n")
}

/// Build `LIST UPS`, enumerating every UPS the server knows about.
pub fn cmd_list_ups() -> String {
    "LIST UPS\n".to_string()
}

/// Build `GET VAR <upsname> <varname>`, e.g. `GET VAR su700 ups.status`, which the
/// RFC gives verbatim as an example returning `VAR su700 ups.status "OB LB"`.
pub fn cmd_get_var(upsname: &str, varname: &str) -> String {
    format!("GET VAR {upsname} {varname}\n")
}

/// Build `INSTCMD <upsname> <cmdname>`, e.g. `INSTCMD su700 test.panel.start`.
pub fn cmd_instcmd(upsname: &str, cmdname: &str) -> String {
    format!("INSTCMD {upsname} {cmdname}\n")
}

/// A parsed one-line response from `upsd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// A bare `OK` (RFC 9271 §4.3.1): the previous command succeeded with no data.
    Ok,
    /// `ERR <name> [extra]` (RFC 9271 §4.3.2), e.g. `ERR UNKNOWN-UPS`.
    Err(String),
    /// `VAR <upsname> <varname> "<value>"`, the answer to `GET VAR`.
    Var {
        upsname: String,
        varname: String,
        value: String,
    },
    /// A line that doesn't match a shape this client currently parses (list framing
    /// like `BEGIN LIST ...` / `END LIST ...`, or an unrecognized response). Callers
    /// that don't need list enumeration can safely ignore this variant.
    Other(String),
}

/// Parse one response line (without its trailing LF).
pub fn parse_response(line: &str) -> Response {
    let line = line.trim_end_matches(['\r', '\n']);
    if line == "OK" {
        return Response::Ok;
    }
    if let Some(rest) = line.strip_prefix("ERR ") {
        return Response::Err(rest.to_string());
    }
    if let Some(rest) = line.strip_prefix("VAR ") {
        // "<upsname> <varname> \"<value>\"": split on the first two spaces, then
        // strip the surrounding quotes from the remainder.
        let mut parts = rest.splitn(3, ' ');
        if let (Some(upsname), Some(varname), Some(quoted)) =
            (parts.next(), parts.next(), parts.next())
        {
            let value = quoted.trim_matches('"').to_string();
            return Response::Var {
                upsname: upsname.to_string(),
                varname: varname.to_string(),
                value,
            };
        }
    }
    Response::Other(line.to_string())
}

/// Turn a parsed [`Response`] into a [`Result`], for callers that just want
/// success/failure rather than the specific shape (e.g. after `INSTCMD`).
pub fn response_to_result(resp: Response) -> Result<()> {
    match resp {
        Response::Ok => Ok(()),
        Response::Err(name) => Err(Error::Protocol(format!("NUT error: {name}"))),
        other => Err(Error::Protocol(format!(
            "unexpected NUT response: {other:?}"
        ))),
    }
}

/// Standard NUT variable names this adapter reads. `battery.charge` and
/// `ups.status` appear verbatim in RFC 9271 itself. `ups.load` and
/// `battery.runtime` are the names used throughout the wider NUT ecosystem
/// (corroborated independently, e.g. in community documentation and issue
/// discussions) but are not directly quoted in the RFC text we have on hand;
/// treat them as high-confidence, not RFC-primary, until confirmed against a
/// real `upsd`.
pub mod var {
    pub const BATTERY_CHARGE: &str = "battery.charge";
    pub const UPS_STATUS: &str = "ups.status";
    pub const UPS_LOAD: &str = "ups.load";
    pub const BATTERY_RUNTIME: &str = "battery.runtime";
}

/// Standard NUT instant-command names this adapter issues. `test.panel.start` is
/// quoted directly in RFC 9271's own example. `beeper.enable`/`beeper.disable` are
/// corroborated independently (NUT community documentation and issue discussions)
/// rather than quoted in the RFC text we have on hand; same confidence caveat as
/// the variable names above.
pub mod instcmd {
    pub const TEST_PANEL_START: &str = "test.panel.start";
    pub const BEEPER_ENABLE: &str = "beeper.enable";
    pub const BEEPER_DISABLE: &str = "beeper.disable";
}

/// Parse the space-separated `ups.status` flag string (RFC 9271's status symbol
/// set: `ALARM BOOST BYPASS CAL CHRG COMM DISCHRG FSD LB NOCOMM OB OFF OL OVER RB
/// TEST TICK TOCK TRIM`) into whether the UPS is on line power or on battery.
/// Returns `None` if neither `OL` nor `OB` is present.
pub fn parse_power_source(status: &str) -> Option<PowerSource> {
    let flags: Vec<&str> = status.split_whitespace().collect();
    if flags.contains(&"OB") {
        Some(PowerSource::Battery)
    } else if flags.contains(&"OL") {
        Some(PowerSource::Line)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_match_rfc_examples() {
        assert_eq!(
            cmd_get_var("su700", "ups.status"),
            "GET VAR su700 ups.status\n"
        );
        assert_eq!(
            cmd_instcmd("su700", "test.panel.start"),
            "INSTCMD su700 test.panel.start\n"
        );
    }

    /// RFC 9271's own example: `GET VAR su700 ups.status` -> `VAR su700 ups.status
    /// "OB LB"`.
    #[test]
    fn parses_rfc_var_example() {
        let resp = parse_response(r#"VAR su700 ups.status "OB LB""#);
        assert_eq!(
            resp,
            Response::Var {
                upsname: "su700".to_string(),
                varname: "ups.status".to_string(),
                value: "OB LB".to_string(),
            }
        );
    }

    #[test]
    fn parses_ok_and_err() {
        assert_eq!(parse_response("OK"), Response::Ok);
        assert_eq!(
            parse_response("ERR UNKNOWN-UPS"),
            Response::Err("UNKNOWN-UPS".to_string())
        );
    }

    #[test]
    fn power_source_from_status_flags() {
        assert_eq!(parse_power_source("OB LB"), Some(PowerSource::Battery));
        assert_eq!(parse_power_source("OL"), Some(PowerSource::Line));
        assert_eq!(parse_power_source("CAL"), None);
    }

    #[test]
    fn response_to_result_maps_ok_and_err() {
        assert!(response_to_result(Response::Ok).is_ok());
        assert!(response_to_result(Response::Err("ACCESS-DENIED".into())).is_err());
    }
}
