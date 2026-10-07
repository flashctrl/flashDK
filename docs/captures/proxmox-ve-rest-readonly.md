# Proxmox VE REST API: read-only wire probes

**Captured:** 2026-10-07 against two owned hosts, on port 8006 over HTTPS (self-signed):
a fresh single node ("Argon", PVE 9.2.2, no guests) and a long-running single node
("Radon", PVE 9.2.21, 5 QEMU VMs of which 3 running and 2 stopped, 6 LXC containers
of which 4 running and 2 stopped). Method: `GET` requests only, sent from a browser
tab that was already signed in to each host's web UI, using that tab's session cookie.
No credentials were handled by this project, nothing was written, and no web UI
source was read. Values below are field *shapes*; hostnames, guest names, IDs and
addresses are deliberately omitted.

All responses are JSON `{"data": ...}` under `/api2/json`, HTTP 200 for every path
tried.

| Path | Result shape |
|---|---|
| `/version` | object: `release`, `version`, `repoid` (strings) |
| `/nodes` | array of node objects, each with a `node` name |
| `/cluster/status` | array; a standalone node returns one element with `type`, `name`, `id`, `nodeid`, `ip`, `online`, `local`, `level` |
| `/cluster/resources` | array; elements carry `id`, `type`, `node`, `status`, `cpu`, `maxcpu`, `mem`, `maxmem`, `disk`, `maxdisk`, `uptime`, ... (one list covering guests, storage and the node) |
| `/nodes/{node}/status` | object: `cpuinfo`, `loadavg`, `swap`, `rootfs`, `pveversion`, `current-kernel`, `boot-info`, `ksm`, memory fields |
| `/nodes/{node}/qemu` | array of VMs: `vmid`, `name`, `status` (`running`/`stopped`), `cpus`, `cpu`, `mem`, `maxmem`, `disk`, `maxdisk`, `uptime`, `netin`, `netout`, `tags`, `pid` (running only), pressure counters |
| `/nodes/{node}/lxc` | array of containers: same family of fields, plus `type`, `diskread`, `diskwrite` |
| `/nodes/{node}/qemu/{vmid}/status/current` | object: `status`, `qmpstatus`, `uptime`, `agent` (number), `cpus`, memory and balloon fields, `nics` (per-tap-device counters), `running-machine`, `proxmox-support` (feature flags) |
| `/nodes/{node}/qemu/{vmid}/config` | object of config keys (about 20+ on the one sampled VM) |
| `/nodes/{node}/lxc/{vmid}/status/current` | object, same family as the VM one |
| `/nodes/{node}/storage` | array: `storage`, `type`, `content`, `total`, `used`, `avail`, `used_fraction`, `active`, `enabled`, `shared` |
| `/nodes/{node}/network` | array: `iface`, `type`, `method`, `method6`, `families`, `active`, `exists`, ... |
| `/nodes/{node}/tasks?limit=2` | array: `upid`, `type`, `status`, `user`, `starttime`, `endtime`, `pid`, `id` |

## What this implies (not yet built)

- A guest is addressed by node plus numeric `vmid`, and a node hosts many of them, so
  this does not fit the one-device-per-adapter `Device` shape; the roadmap's idea of a
  thin collection type still holds.
- `status` gives a clean power state for guests; counts and fields above are enough for a
  read-only inventory and live stats.
- Same quirk as elsewhere in this repo: a browser-cookie probe proves the read paths,
  not how a native client authenticates. Auth (API token versus ticket plus CSRF for
  writes) still has to be taken from Proxmox's official API documentation and verified
  against a host before any write call is built; nothing in this probe touched it.
