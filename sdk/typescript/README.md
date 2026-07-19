# Pi Extension TypeScript SDK Candidate

This private `0.1.0-candidate.1` package is the repository-owned precursor to
the public extension SDK planned for `0.4.5`. The WIT file under
`contracts/extensions/0.1.0/` is authoritative; Jco generates ambient guest
types into `target/extension-sdk/generated/` for every conformance run.

The supported authoring subset is strict TypeScript compiled to an ES2022 ESM
bundle. Node built-ins, native addons, dynamic imports, CommonJS `require`,
runtime code generation, and ambient WASI are unsupported. Installed packages
contain only the resulting Wasm Component, manifest, lock, and declared data
resources.

Host workspace, model, structured-process, and UI families are capability
imports. Session association is an operation lease scope; generic extension
session state and facts are intentionally deferred to `0.4.3`.

Run the offline internal workflow from the workspace root:

```bash
scripts/extension-sdk.sh run-conformance
```

The script uses exact versions from `package-lock.json`. It restores tools with
`npm ci --offline` into `target/extension-sdk/toolchain/`; set
`PI_RUST_EXTENSION_NPM_CACHE` to a populated locked npm cache when the default
npm cache is unavailable. No generated files or Node dependencies are written
under this SDK source directory.
