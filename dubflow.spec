# PyInstaller definition. Build from the repository root with:
#   python -m PyInstaller --clean --noconfirm dubflow.spec
#
# Lightweight GUI build: AI dependencies are loaded from a user-selected
# Python Runtime and models remain in the user-selected Workspace.

from pathlib import Path


app_sources = [
    (str(source), str(Path("app_runtime") / source.parent))
    for source in Path("app").rglob("*.py")
]

analysis = Analysis(
    ["app/__main__.py"],
    pathex=["."],
    binaries=[],
    # Package source files explicitly so local __pycache__, logs, and other
    # development artifacts can never leak into release installers.
    datas=app_sources,
    hiddenimports=[],
    excludes=[
        "accelerate",
        "anyio",
        "boto3",
        "botocore",
        "bitsandbytes",
        "datasets",
        "f5_tts",
        "gradio",
        "google.cloud",
        "huggingface_hub",
        "jupyter",
        "notebook",
        "numpy",
        "pandas",
        "pyarrow",
        "pytest",
        "rich",
        "scipy",
        "sentry_sdk",
        "sklearn",
        "soundfile",
        "tensorboard",
        "torch",
        "torchaudio",
        "transformers",
        "vocos",
        "x_transformers",
    ],
    noarchive=False,
)
pyz = PYZ(analysis.pure)
exe = EXE(
    pyz,
    analysis.scripts,
    [],
    exclude_binaries=True,
    name="DubFlow",
    console=False,
    disable_windowed_traceback=False,
)
bundle = COLLECT(
    exe,
    analysis.binaries,
    analysis.datas,
    strip=False,
    upx=False,
    name="DubFlow",
)
