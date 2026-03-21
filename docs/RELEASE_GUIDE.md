# Publishing Releases

## Release Artifacts

This project produces **3 separate distributables**:

| Artifact | Type | Target | Install Method |
|----------|------|--------|----------------|
| `ifol-render-windows-x64.zip` | Binaries | Backend devs | GitHub Releases download |
| `@danphong23/ifol-render-wasm` | NPM package | Web devs | `npm install @danphong23/ifol-render-wasm` |
| `@danphong23/ifol-render-sdk` | NPM package | Web devs | `npm install @danphong23/ifol-render-sdk` |

> [!IMPORTANT]
> The CLI binary does **NOT** bundle FFmpeg. Users must have FFmpeg installed on their system or pass its path via `--ffmpeg "C:/path/to/ffmpeg.exe"`.

## Automated Build

Run the PowerShell script from the project root:
```powershell
.\scripts\build_release.ps1
```
This will:
1. Compile CLI + Studio in `--release` mode → zip into `release_builds/`
2. Build WASM via `wasm-pack` → patch scoped name → pack `.tgz`
3. Build SDK TypeScript → pack `.tgz`

## Publishing Workflow

### Step 1: Publish WASM to NPM (first!)
```bash
cd crates/wasm/pkg
npm publish --access public
```
*(The SDK depends on WASM, so WASM must be published first.)*

### Step 2: Publish SDK to NPM
```bash
cd sdk
npm publish --access public
```

### Step 3: Upload Binaries to GitHub Releases
1. Go to your [GitHub repository](https://github.com/nicengi/ifol-render) → **Releases** → **Draft a new release**
2. Tag: `v0.2.0`
3. Attach `release_builds/ifol-render-windows-x64.zip`
4. Publish

## Versioning Strategy

| Component | Version File | Current |
|-----------|-------------|---------|
| Rust workspace (CLI, Studio, Core, Audio) | `Cargo.toml` `workspace.package.version` | `0.2.0` |
| WASM NPM package | `scripts/patch_wasm_pkg.ps1` | `0.2.0` |
| SDK NPM package | `sdk/package.json` | `0.3.0` |

> [!TIP]
> WASM and Rust versions should stay in sync (both are compiled from the same Rust source). The SDK version can differ because it's a TypeScript layer that may evolve independently.

## Local Development (Without Publishing)

For local testing, developers can install the `.tgz` files directly:
```bash
npm install ./release_builds/danphong23-ifol-render-wasm-0.2.0.tgz
npm install ./release_builds/danphong23-ifol-render-sdk-0.3.0.tgz
```

Or for SDK development, use the `file:` protocol temporarily:
```bash
# In sdk/package.json, temporarily change:
"@danphong23/ifol-render-wasm": "file:../crates/wasm/pkg"
```

## NPM Login
Before publishing for the first time:
```bash
npm login
# Enter your npmjs.com credentials
```

## Setting Author
Author metadata is defined in:
- `Cargo.toml` → `workspace.package.authors = ["danphong23"]`
- `sdk/package.json` → `"author": "danphong23"`
- `scripts/patch_wasm_pkg.ps1` → patches author into WASM package
