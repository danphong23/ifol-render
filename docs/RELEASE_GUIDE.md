# Publishing Releases

This project consists of two different types of distributables:
1. **Rust Binaries** (`ifol-render-cli.exe` and `ifol-render-studio.exe`) -> Shipped via **GitHub Releases**.
2. **Typescript SDK** (`ifol-render-sdk`) -> Shipped via **NPM** (Node Package Manager).

## 1. Automated Build Script
To make releasing easy, run the included PowerShell script:
```powershell
.\scripts\build_release.ps1
```
This script will:
- Build the Rust binaries in `--release` mode (max optimization).
- Zip the binaries into `release_builds/ifol-render-windows-x64.zip`.
- Build the WebAssembly core (`crates/wasm/pkg`).
- Build the Typescript SDK (`sdk/dist`).
- Pack the SDK into an NPM tarball `release_builds/ifol-render-sdk-0.3.0.tgz`.

## 2. Publishing to GitHub
When developers want to download your CLI or Studio, they usually go to the `Releases` tab on GitHub.
1. Go to your repository on GitHub.
2. Click **Releases** on the right side.
3. Click **Draft a new release**.
4. Choose a tag (e.g., `v0.2.0`).
5. Write the release notes (e.g., "Added new AudioScene architecture and stutter-free web preview").
6. **Drag and drop** the `ifol-render-windows-x64.zip` file from your `release_builds` folder into the attached binaries section.
7. Click **Publish release**.

## 3. Publishing the SDK to NPM
Developers building web apps want to install your SDK using `npm install ifol-render-sdk`.
1. Make sure you have an account on [npmjs.com](https://www.npmjs.com/).
2. Open your terminal and log in:
   ```bash
   npm login
   ```
3. Navigate to the `sdk` folder:
   ```bash
   cd sdk
   ```
4. Publish the package:
   ```bash
   npm publish
   ```
*(Note: If the package name `ifol-render-sdk` is already taken globally on NPM by someone else, you may need to scope it like `@danphong23/ifol-render-sdk` in `package.json`).*

## Local Testing (Without Publishing)
If another developer wants to use your SDK *without* you publishing it to NPM, they can install the `.tgz` file directly!
Just send them the `ifol-render-sdk-0.3.0.tgz` file, and they can run:
```bash
npm install /path/to/ifol-render-sdk-0.3.0.tgz
```
