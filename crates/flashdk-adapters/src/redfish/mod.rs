//! A DMTF Redfish client covering `Power` and `VirtualMedia`, aimed at enterprise
//! BMCs (Dell iDRAC, HPE iLO) rather than the hobbyist HDMI-capture KVMs the rest of
//! this crate targets. Unlike those, Redfish is a published, vendor-neutral REST/JSON
//! standard, so this adapter is implemented to specification rather than
//! reverse-engineered; see `protocol.rs` and `PROVENANCE.md` for exactly which fact
//! traces to which official DMTF document.
//!
//! **Not yet verified against real hardware**: this project has neither an iDRAC
//! nor an iLO unit to test against at the time of writing. Every request shape is
//! taken from the official schema and unit-tested against it in `protocol.rs`, but
//! the end-to-end HTTP flow below (login, discovery, action dispatch) has not been
//! exercised against a live BMC. Treat it the way `docs/STATE.md` treats other
//! unverified claims: real code, not yet proven live.
//!
//! Deliberately standalone from the [`flashdk_core::Device`] umbrella trait, the
//! same reasoning as `nut`: a BMC's `Power` and `VirtualMedia` are real, but it has
//! no keyboard or mouse the way a KVM does, and its own graphical console is
//! per-vendor proprietary, not part of the Redfish standard, so `Hid` doesn't apply
//! here at all.
//!
//! Doesn't implement `Device`/`Vendor` (that enum is scoped to the HDMI-capture KVMs
//! `Kvm` dispatches over); a single `RedfishBmc` deliberately serves any
//! Redfish-conformant BMC, iDRAC or iLO alike, which is the point of building to the
//! standard rather than per-vendor.

pub mod protocol;

use std::sync::Arc;

use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Error, Result};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::tls_pin::{self, MemoryPinStore, PinStore};
use protocol::{
    insert_media_body, login_body, power_state_to_bool, reset_body, reset_type_for,
    SystemPowerState, VirtualMediaResource, AUTH_TOKEN_HEADER,
};

/// A connected Redfish BMC, scoped to the first `ComputerSystem` and the first
/// manager's `VirtualMedia` collection discovered at login (most BMCs expose
/// exactly one of each; a multi-system chassis would need a richer API than this
/// first pass offers).
pub struct RedfishBmc {
    base_url: String,
    http: reqwest::Client,
    token: Mutex<Option<String>>,
    system_uri: Mutex<Option<String>>,
    vm_uris: Mutex<Vec<String>>,
}

impl RedfishBmc {
    /// Log in to `host` (e.g. `"10.0.10.30"`) and discover its first `ComputerSystem`
    /// and `VirtualMedia` collection.
    ///
    /// Redfish services normally present a certificate signed by the vendor's own
    /// internal CA (iDRAC, iLO) rather than one a public trust store recognizes, so
    /// this uses the same trust-on-first-use pinning as PiKVM; see
    /// [`crate::tls_pin`].
    pub async fn connect(host: &str, username: &str, password: &str) -> Result<Self> {
        Self::connect_with_pin_store(
            host,
            username,
            password,
            Arc::new(MemoryPinStore::default()),
        )
        .await
    }

    /// Like [`Self::connect`], but pins are read from and written to `store`.
    pub async fn connect_with_pin_store(
        host: &str,
        username: &str,
        password: &str,
        store: Arc<dyn PinStore>,
    ) -> Result<Self> {
        let http = tls_pin::tofu_client(host, store).map_err(Error::Transport)?;
        let base_url = format!("https://{host}");

        let mut bmc = Self {
            base_url,
            http,
            token: Mutex::new(None),
            system_uri: Mutex::new(None),
            vm_uris: Mutex::new(Vec::new()),
        };
        bmc.login(username, password).await?;
        bmc.discover().await?;
        Ok(bmc)
    }

    /// `POST /redfish/v1/SessionService/Sessions`, storing the `X-Auth-Token`
    /// response header (DSP0266's session-login flow; see `PROVENANCE.md`).
    async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        let resp = self
            .http
            .post(format!(
                "{}/redfish/v1/SessionService/Sessions",
                self.base_url
            ))
            .json(&login_body(username, password))
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Error::Auth(format!(
                "Redfish session login rejected: HTTP {}",
                resp.status()
            )));
        }

        let token = resp
            .headers()
            .get(AUTH_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .ok_or_else(|| Error::Auth("Redfish login response had no X-Auth-Token".into()))?;

        *self.token.lock().await = Some(token);
        Ok(())
    }

    /// Walk `/redfish/v1/` -> `Systems`/`Managers` -> first member of each, caching
    /// the system URI and every `VirtualMedia` member URI found under the first
    /// manager.
    async fn discover(&self) -> Result<()> {
        let root = self.get("/redfish/v1/").await?;
        let systems_uri = odata_id(&root["Systems"])
            .ok_or_else(|| Error::Protocol("Redfish root has no Systems collection".to_string()))?;
        let systems = self.get(&systems_uri).await?;
        let system_uri = first_member(&systems)
            .ok_or_else(|| Error::Protocol("Redfish Systems collection is empty".to_string()))?;
        *self.system_uri.lock().await = Some(system_uri);

        // VirtualMedia is commonly exposed under the first Manager. Absent means
        // this BMC just doesn't offer it, not a discovery failure.
        if let Some(managers_uri) = odata_id(&root["Managers"]) {
            let managers = self.get(&managers_uri).await?;
            if let Some(manager_uri) = first_member(&managers) {
                let manager = self.get(&manager_uri).await?;
                if let Some(vm_collection_uri) = odata_id(&manager["VirtualMedia"]) {
                    let vm_collection = self.get(&vm_collection_uri).await?;
                    let uris: Vec<String> = vm_collection["Members"]
                        .as_array()
                        .map(|members| members.iter().filter_map(odata_id).collect())
                        .unwrap_or_default();
                    *self.vm_uris.lock().await = uris;
                }
            }
        }
        Ok(())
    }

    /// Attach the bearer of this session (`X-Auth-Token`, not HTTP `Authorization`;
    /// DSP0266's own scheme) to a request.
    async fn authed(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &*self.token.lock().await {
            Some(token) => rb.header(AUTH_TOKEN_HEADER, token),
            None => rb,
        }
    }

    async fn get(&self, path_or_uri: &str) -> Result<Value> {
        let rb = self.http.get(self.absolute(path_or_uri));
        let resp = self
            .authed(rb)
            .await
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        resp.json()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))
    }

    async fn post(&self, path_or_uri: &str, body: &Value) -> Result<()> {
        let rb = self.http.post(self.absolute(path_or_uri)).json(body);
        let resp = self
            .authed(rb)
            .await
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(Error::Protocol(format!(
                "Redfish action rejected: HTTP {}",
                resp.status()
            )))
        }
    }

    fn absolute(&self, path_or_uri: &str) -> String {
        if path_or_uri.starts_with("http") {
            path_or_uri.to_string()
        } else {
            format!("{}{}", self.base_url, path_or_uri)
        }
    }
}

/// Pull an `"@odata.id"` string out of a link object, the shape every Redfish
/// reference (`Systems`, `Managers`, a collection `Members` entry, ...) uses.
fn odata_id(v: &Value) -> Option<String> {
    v.get("@odata.id")?.as_str().map(str::to_string)
}

/// The first entry of a Redfish collection's `Members` array, as an `@odata.id`.
fn first_member(collection: &Value) -> Option<String> {
    collection
        .get("Members")?
        .as_array()?
        .first()
        .and_then(odata_id)
}

impl Power for RedfishBmc {
    async fn action(&self, action: PowerAction) -> Result<()> {
        let system_uri = self
            .system_uri
            .lock()
            .await
            .clone()
            .ok_or(Error::NotSupported("no ComputerSystem discovered"))?;
        let system = self.get(&system_uri).await?;
        let target = system["Actions"]["#ComputerSystem.Reset"]["target"]
            .as_str()
            .ok_or(Error::NotSupported(
                "this system does not advertise #ComputerSystem.Reset",
            ))?
            .to_string();
        self.post(&target, &reset_body(reset_type_for(action)))
            .await
    }

    async fn state(&self) -> Result<PowerState> {
        let system_uri = self
            .system_uri
            .lock()
            .await
            .clone()
            .ok_or(Error::NotSupported("no ComputerSystem discovered"))?;
        let system: SystemPowerState = serde_json::from_value(self.get(&system_uri).await?)
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(PowerState {
            powered: system.power_state.as_deref().and_then(power_state_to_bool),
            // Redfish's ComputerSystem model has no generic HDD-activity signal;
            // that's platform-specific telemetry (e.g. Storage/Drive health), not
            // part of this trait's scope.
            hdd_activity: None,
        })
    }
}

impl VirtualMedia for RedfishBmc {
    async fn list(&self) -> Result<Vec<MediaImage>> {
        let uris = self.vm_uris.lock().await.clone();
        let mut out = Vec::with_capacity(uris.len());
        for uri in uris {
            let vm: VirtualMediaResource = serde_json::from_value(self.get(&uri).await?)
                .map_err(|e| Error::Protocol(e.to_string()))?;
            out.push(MediaImage {
                name: vm.image_name.unwrap_or(vm.id),
                size: None,
                mounted: vm.inserted.unwrap_or(false),
            });
        }
        Ok(out)
    }

    /// Redfish doesn't pre-list mountable images the way NanoKVM/PiKVM's local
    /// storage does; `InsertMedia` takes an image URI directly. `name` here is
    /// therefore that URI, inserted into the first `VirtualMedia` slot that isn't
    /// already occupied.
    async fn mount(&self, name: &str) -> Result<()> {
        let uris = self.vm_uris.lock().await.clone();
        for uri in uris {
            let vm: VirtualMediaResource = serde_json::from_value(self.get(&uri).await?)
                .map_err(|e| Error::Protocol(e.to_string()))?;
            if vm.inserted != Some(true) {
                let resource = self.get(&uri).await?;
                let target = resource["Actions"]["#VirtualMedia.InsertMedia"]["target"]
                    .as_str()
                    .ok_or(Error::NotSupported(
                        "this VirtualMedia slot has no InsertMedia action",
                    ))?
                    .to_string();
                return self.post(&target, &insert_media_body(name)).await;
            }
        }
        Err(Error::NotSupported(
            "no free VirtualMedia slot to insert into",
        ))
    }

    async fn unmount(&self) -> Result<()> {
        let uris = self.vm_uris.lock().await.clone();
        for uri in uris {
            let resource = self.get(&uri).await?;
            let vm: VirtualMediaResource = serde_json::from_value(resource.clone())
                .map_err(|e| Error::Protocol(e.to_string()))?;
            if vm.inserted == Some(true) {
                let target = resource["Actions"]["#VirtualMedia.EjectMedia"]["target"]
                    .as_str()
                    .ok_or(Error::NotSupported(
                        "this VirtualMedia slot has no EjectMedia action",
                    ))?
                    .to_string();
                return self.post(&target, &Value::Object(Default::default())).await;
            }
        }
        // Nothing inserted anywhere is a no-op success, not an error: the
        // post-condition ("nothing mounted") already holds.
        Ok(())
    }
}
