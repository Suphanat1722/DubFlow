# DubFlow Toolchain

## Version ที่ใช้ใน Phase 0

| Tool | Version | หมายเหตุ |
|---|---|---|
| Windows | 10/11 (x64) | target platform เท่านั้น |
| Rust (rustup stable) | 1.97.1 (MSVC) | `x86_64-pc-windows-msvc` |
| MSVC Build Tools | 2022 17.14 (14.44.35207) | C++ toolset สําหรับ link |
| Node.js | 24.18.0 | npm 11.16.0 |
| Python | 3.11.15 | worker sidecar |
| Tauri | 2.x | จัดการผ่าน Cargo/npm |
| React | 19.x | UI |
| Vite | 7.x | dev server/build |

ตรวจสอบ:

```powershell
rustc --version
node --version
py -3.11 --version
```

## Windows prerequisites

- WebView2 Runtime (ติดตั้งกับ Windows 10/11 ตามปกติ)
- MSVC C++ build tools จาก Visual Studio Installer
- rustup toolchain `stable-x86_64-pc-windows-msvc`

## Development commands

จากโฟลเดอร์ `desktop/`:

```powershell
npm install
npm run tauri dev
```

Python worker (จาก `python-worker/`):

```powershell
py -3.11 -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -r requirements-dev.txt
python -m pytest
```
