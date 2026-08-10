# DubFlow Development Guide

## โครงสร้าง

```text
desktop/          Tauri 2 + React/TypeScript desktop shell
python-worker/    Python 3.11 sidecar (JSON-RPC)
specs/            IPC, project schema, TTS provider contract
docs/             project/phase documentation
```

## Desktop

```powershell
cd desktop
npm install
npm run tauri dev
```

ตรวจ:

```powershell
npm run lint
npm run test
npm run build
cd src-tauri
cargo test
cargo check
```

## Python worker

```powershell
cd python-worker
py -3.11 -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -r requirements-dev.txt
python -m pytest
```

## CI

GitHub Actions workflow ที่ `desktop/../.github/workflows/ci.yml` รัน
TypeScript lint/build, Rust test/check และ Python test บน Windows
