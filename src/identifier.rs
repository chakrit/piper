use anyhow::Result;

pub fn validate(value: &str, name: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if !valid {
        anyhow::bail!("{name} must use only ASCII letters, digits, hyphens, or underscores")
    }

    Ok(())
}
