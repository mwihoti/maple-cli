//! Maple keys are scoped to a validated endpoint and stored only in the OS store.
//! Never format the store's errors: a backend error may carry secret bytes.

use codex_keyring_store::KeyringStore;

const SERVICE: &str = "Maple Code";

fn account(base_url: &str) -> Result<String, String> {
    let mut url = codex_http_client::validate_endpoint_url(base_url, "Maple base_url")?;
    if url.query().is_some() {
        return Err("Maple base_url must not contain query parameters".into());
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url.to_string())
}

pub fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(
            "Maple API key must be non-empty and contain no whitespace or control characters"
                .into(),
        );
    }
    Ok(())
}

pub fn load(store: &dyn KeyringStore, base_url: &str) -> Result<Option<String>, String> {
    let key = store.load(SERVICE, &account(base_url)?).map_err(|_| {
        "Maple system credential store is unavailable; unlock it or use MAPLE_API_KEY".to_owned()
    })?;
    if let Some(key) = &key {
        validate_key(key)?;
    }
    Ok(key)
}

pub fn save(store: &dyn KeyringStore, base_url: &str, key: &str) -> Result<(), String> {
    validate_key(key)?;
    store.save(SERVICE, &account(base_url)?, key).map_err(|_| {
        "Could not save Maple key to the system credential store; no plaintext fallback is used"
            .into()
    })
}

pub fn delete(store: &dyn KeyringStore, base_url: &str) -> Result<bool, String> {
    store
        .delete(SERVICE, &account(base_url)?)
        .map_err(|_| "Could not remove Maple key from the system credential store".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_keyring_store::tests::MockKeyringStore;

    #[test]
    fn credential_roundtrip_scoped_to_canonical_endpoint_and_logout() {
        let store = MockKeyringStore::default();
        save(&store, "http://127.0.0.1:8080/v1/", "fixture-key").unwrap();
        assert_eq!(
            load(&store, "http://127.0.0.1:8080/v1").unwrap().as_deref(),
            Some("fixture-key")
        );
        assert!(load(&store, "http://127.0.0.1:8081/v1").unwrap().is_none());
        assert!(delete(&store, "http://127.0.0.1:8080/v1").unwrap());
        assert!(load(&store, "http://127.0.0.1:8080/v1").unwrap().is_none());
    }

    #[test]
    fn rejects_insecure_endpoints_and_invalid_keys_without_echoing_them() {
        let store = MockKeyringStore::default();
        for url in [
            "http://remote.example/v1",
            "https://user:fixture-key@example.com/v1",
            "https://example.com/v1?api_key=fixture-key",
        ] {
            assert!(
                !save(&store, url, "fixture-key")
                    .unwrap_err()
                    .contains("fixture-key")
            );
        }
        for key in ["", "fixture-key\n", "fixture key"] {
            assert!(
                !save(&store, "http://127.0.0.1:8080/v1", key)
                    .unwrap_err()
                    .contains("fixture-key")
            );
        }
    }
}
