use sha2::{Digest, Sha256};

pub fn generate_app_id(name: &str, source_hint: Option<&str>) -> String {
    let slug = slugify(name);
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update([0]);
    if let Some(source) = source_hint {
        hasher.update(source.as_bytes());
    }
    let digest = hasher.finalize();
    let suffix = digest
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    let mut prefix = slug;
    if prefix.len() > 48 {
        prefix.truncate(48);
        prefix = prefix.trim_end_matches('-').to_string();
    }

    format!("{prefix}-{suffix}")
}

pub fn is_valid_app_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 3 || bytes.len() > 80 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return false;
    }
    if !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit() {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len().min(48));
    let mut last_dash = false;

    for ch in name.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else {
            None
        };

        match normalized {
            Some(ch) => {
                out.push(ch);
                last_dash = false;
            }
            None if !last_dash && !out.is_empty() => {
                out.push('-');
                last_dash = true;
            }
            None => {}
        }
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "app".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_app_id, is_valid_app_id};

    #[test]
    fn generated_app_id_is_stable_and_valid() {
        let first = generate_app_id("Example App", Some("setup.exe"));
        let second = generate_app_id("Example App", Some("setup.exe"));
        assert_eq!(first, second);
        assert!(is_valid_app_id(&first));
        assert!(first.starts_with("example-app-"));
    }
}
