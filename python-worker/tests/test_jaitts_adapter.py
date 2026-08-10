import pytest

from dubflow_worker.tts import SynthesisSettings, settings_hash


def test_settings_hash_is_deterministic_and_sorted():
    a = SynthesisSettings(nfe_step=32, cfg_strength=2.0)
    b = SynthesisSettings(cfg_strength=2.0, nfe_step=32)
    assert settings_hash(a) == settings_hash(b)
    assert len(settings_hash(a)) == 64


def test_settings_hash_changes_with_speed():
    assert settings_hash(SynthesisSettings(speed=1.0)) != settings_hash(
        SynthesisSettings(speed=1.25)
    )


def test_worker_tts_contract_imports_without_cuda():
    pytest.importorskip("torch")
    pytest.importorskip("f5_tts")
    from dubflow_worker.tts.jaitts_f5tts import PROVIDER_ID, PROVIDER_VERSION

    assert PROVIDER_ID == "jaitts-f5tts"
    assert PROVIDER_VERSION == "1.1.22"
