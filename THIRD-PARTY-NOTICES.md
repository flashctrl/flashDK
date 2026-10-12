# Third-party notices

Licences of dependencies that need a note beyond `docs/credits.md`. The policy: keep
dependencies current, and where a licence outside the usual allow-list appears, prove the
code is not used or shipped, allow it in `deny.toml` with a written justification, and
record it here. If non-use cannot be proven, the dependency is not allowed.

## webpki-root-certs 1.0.x: CDLA-Permissive-2.0 (data licence)

- **What it is:** the Mozilla CA root-certificate bundle, as data.
- **Status:** in `Cargo.lock` only. **Not compiled into any shipped target.** It is not
  used by flashDK.
- **Path:** `flashdk-adapters` -> `reqwest 0.13` -> `rustls-platform-verifier 0.7` ->
  `webpki-root-certs`, through a dependency declared for `cfg(target_arch = "wasm32")` only.
- **Evidence of non-use (checked 2026-11-24):**
  - `cargo tree -i webpki-root-certs --target <t>` prints nothing for `aarch64-linux-android`,
    `x86_64-linux-android`, `aarch64-apple-ios`, `aarch64-apple-ios-sim`,
    `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`. It
    appears only with `--target all`.
  - The release `libflashdk.so` for `arm64-v8a` (cargo-ndk, API 26) contains none of the
    CA subject names searched for (ISRG, DigiCert, GlobalSign, Amazon Root, Let's Encrypt,
    Sectigo, USERTrust, Baltimore). The same search on a control binary that embeds the
    bundle finds 4 to 32 matches per name, so the search can detect it.
  - flashDK never relies on a CA bundle: every TLS connection uses its own pinned,
    trust-on-first-use verifier (`tls_pin.rs`), with no CA roots.
- **Re-verify** when `reqwest`, `rustls-platform-verifier` or the targets change.
