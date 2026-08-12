# PyInstaller definition. Build from the repository root with:
#   python -m PyInstaller --clean --noconfirm dubflow.spec
#
# Let PyInstaller's package hooks collect PyTorch/CUDA and Qt binaries. Avoid
# collect_all(f5_tts): that would also bundle its training, Gradio, evaluation,
# and Triton-server tools. Vocos creates these classes dynamically from YAML,
# so they are the only inference modules that need explicit hidden imports.
from PyInstaller.utils.hooks import collect_data_files


hiddenimports = [
    "vocos.feature_extractors",
    "vocos.heads",
    "vocos.models",
]

analysis = Analysis(
    ["app/__main__.py"],
    pathex=["."],
    binaries=[],
    datas=collect_data_files("x_transformers", include_py_files=True, includes=["*.py"]),
    hiddenimports=hiddenimports,
    excludes=[
        "boto3",
        "botocore",
        "bitsandbytes",
        "datasets",
        "gradio",
        "google.cloud",
        "jupyter",
        "notebook",
        "pandas",
        "pyarrow",
        "pytest",
        "sentry_sdk",
        "sklearn",
        "tensorboard",
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
