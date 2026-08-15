// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

const TOKEN_PREFIX: &str = "moor_";
const TOKEN_RANDOM_BYTES: usize = 32;

pub fn generate() -> Result<String, String> {
    let mut bytes = [0_u8; TOKEN_RANDOM_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("failed to generate MCP token: {error}"))?;
    Ok(format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_has_prefix_and_no_whitespace() {
        let token = generate().expect("token should generate");

        assert!(token.starts_with(TOKEN_PREFIX));
        assert!(!token.chars().any(char::is_whitespace));
    }
}
