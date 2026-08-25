//! A NUT (Network UPS Tools) `upsd` client, implementing
//! [`flashdk_core::ups::UpsStatus`] for an APC Back-UPS (or any UPS) reached
//! through a NUT server rather than directly: a consumer UPS like the Back-UPS is
//! USB-only with no network port of its own, so `upsd` running on a nearby host
//! with the UPS attached is the network path to it.
//!
//! Implemented entirely from RFC 9271 (the IETF-published NUT network protocol)
//! and NUT's official manual pages; see `protocol.rs` and `PROVENANCE.md` for
//! exactly which fact came from which source. **Not yet verified against a real
//! `upsd`**: this codebase has no UPS or NUT server to test against at the time of
//! writing. The protocol encoder/parser is unit-tested against RFC 9271's own
//! literal example text (see `protocol.rs`'s tests), which is real verification of
//! the wire format, but the end-to-end TCP client below has not been exercised
//! against a running server. Treat it the way `docs/STATE.md` treats other
//! unverified claims: real code, not yet proven live.

pub mod protocol;

use flashdk_core::ups::{UpsCommand, UpsState, UpsStatus};
use flashdk_core::{Error, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use protocol::{
    cmd_get_var, cmd_instcmd, cmd_password, cmd_username, instcmd, parse_response, var, Response,
};

/// NUT's IANA-registered port (RFC 9271 §3): `upsd` listens here by default.
pub const DEFAULT_PORT: u16 = 3493;

/// A connection to a NUT server, scoped to one named UPS on that server (a single
/// `upsd` can serve several).
pub struct NutUps {
    upsname: String,
    stream: Mutex<BufReader<TcpStream>>,
}

impl NutUps {
    /// Connect to `upsd` at `host:port` and authenticate as `username`/`password`
    /// (RFC 9271 §4.2: `USERNAME` then `PASSWORD`, each acknowledged with `OK`),
    /// scoping subsequent calls to `upsname`.
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        upsname: &str,
    ) -> Result<Self> {
        let tcp = TcpStream::connect((host, port))
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let mut reader = BufReader::new(tcp);

        write_line(&mut reader, &cmd_username(username)).await?;
        expect_ok(&mut reader).await?;
        write_line(&mut reader, &cmd_password(password)).await?;
        expect_ok(&mut reader).await?;

        Ok(Self {
            upsname: upsname.to_string(),
            stream: Mutex::new(reader),
        })
    }

    /// Issue `GET VAR <upsname> <varname>` and return the value string, or `None`
    /// if the server doesn't have that variable for this UPS (treated as absent
    /// data, not a hard error, since not every UPS reports every variable).
    async fn get_var(&self, varname: &str) -> Option<String> {
        let mut stream = self.stream.lock().await;
        write_line(&mut stream, &cmd_get_var(&self.upsname, varname))
            .await
            .ok()?;
        let line = read_line(&mut stream).await.ok()?;
        match parse_response(&line) {
            Response::Var { value, .. } => Some(value),
            _ => None,
        }
    }
}

impl UpsStatus for NutUps {
    async fn state(&self) -> Result<UpsState> {
        let status = self.get_var(var::UPS_STATUS).await;
        let source = status.as_deref().and_then(protocol::parse_power_source);
        let charge_percent = self
            .get_var(var::BATTERY_CHARGE)
            .await
            .and_then(|v| v.parse().ok());
        let load_percent = self
            .get_var(var::UPS_LOAD)
            .await
            .and_then(|v| v.parse().ok());
        let runtime_seconds = self
            .get_var(var::BATTERY_RUNTIME)
            .await
            .and_then(|v| v.parse().ok());

        Ok(UpsState {
            source,
            charge_percent,
            load_percent,
            runtime_seconds,
        })
    }

    async fn command(&self, cmd: UpsCommand) -> Result<()> {
        let name = match cmd {
            UpsCommand::SelfTest => instcmd::TEST_PANEL_START,
            UpsCommand::MuteBeeper => instcmd::BEEPER_DISABLE,
            UpsCommand::UnmuteBeeper => instcmd::BEEPER_ENABLE,
        };
        let mut stream = self.stream.lock().await;
        write_line(&mut stream, &cmd_instcmd(&self.upsname, name)).await?;
        let line = read_line(&mut stream).await?;
        protocol::response_to_result(parse_response(&line))
    }
}

async fn write_line(stream: &mut BufReader<TcpStream>, line: &str) -> Result<()> {
    stream
        .get_mut()
        .write_all(line.as_bytes())
        .await
        .map_err(|e| Error::Transport(e.to_string()))
}

async fn read_line(stream: &mut BufReader<TcpStream>) -> Result<String> {
    let mut line = String::new();
    stream
        .read_line(&mut line)
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    if line.is_empty() {
        return Err(Error::Transport("NUT server closed the connection".into()));
    }
    Ok(line)
}

async fn expect_ok(stream: &mut BufReader<TcpStream>) -> Result<()> {
    let line = read_line(stream).await?;
    match parse_response(&line) {
        Response::Ok => Ok(()),
        Response::Err(name) => Err(Error::Auth(format!("NUT auth rejected: {name}"))),
        other => Err(Error::Protocol(format!(
            "unexpected NUT response during auth: {other:?}"
        ))),
    }
}
