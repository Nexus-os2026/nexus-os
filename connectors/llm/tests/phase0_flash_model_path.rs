//! P0-002C5C: the flash provider takes its model file only from an absolute
//! path. There is no default relative to the process working directory, so a
//! relative value, or none, leaves the provider unavailable.

use nexus_connectors_llm::gateway::{select_provider, ProviderSelectionConfig};

fn flash(provider: &str, model_path: Option<&str>) -> ProviderSelectionConfig {
    ProviderSelectionConfig {
        provider: Some(provider.to_string()),
        flash_model_path: model_path.map(str::to_string),
        ..Default::default()
    }
}

#[test]
fn flash_provider_needs_an_absolute_model_path() {
    for provider in ["flash", "flash-infer", "local-gguf"] {
        for model_path in [
            None,
            Some(""),
            Some("flash-local"),
            Some("./models/m.gguf"),
            Some("../m.gguf"),
        ] {
            let error = match select_provider(&flash(provider, model_path)) {
                Ok(_) => panic!("{provider} {model_path:?}: a provider was built"),
                Err(error) => error.to_string(),
            };
            assert!(
                error.contains("needs an absolute model path"),
                "{provider} {model_path:?}: {error}"
            );
        }
    }
    let absolute = std::env::temp_dir().join("nexus-c5c-absent-model.gguf");
    let config = flash("local-gguf", Some(&absolute.to_string_lossy()));
    assert!(select_provider(&config).is_ok());
}
