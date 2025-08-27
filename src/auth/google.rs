// src/auth/google.rs

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;

// Struct AuthRequest
#[derive(Deserialize)]
pub struct AuthRequest {
    pub token: String,
}

// Struct to hold the claims from the Google ID token
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleClaims {
    pub iss: String, // Issuer
    pub aud: String, // Audience (your client ID)
    pub exp: usize,  // Expiration time
    pub sub: String, // User's unique Google ID
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

// Structs to deserialize Google's public keys (JWKS)
#[derive(Debug, Deserialize)]
struct Jwk {
    keys: Vec<JsonWebKey>,
}

#[derive(Debug, Deserialize)]
struct JsonWebKey {
    kid: String, // Key ID
    alg: String, // Algorithm (e.g., "RS256")
    n: String,   // Modulus
    e: String,   // Exponent
}

/// Verifies a Google ID token by fetching public keys and checking claims.
/// This follows the manual verification steps outlined by Google.
pub async fn verify_google_token(token_str: &str, client_id: &str) -> Result<GoogleClaims, String> {
    // 1. Fetch Google's public keys
    // In a real app, you should cache these keys based on the Cache-Control header.
    let client = Client::new();
    let jwks_url = "https://www.googleapis.com/oauth2/v3/certs";
    let jwks = client
        .get(jwks_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch JWKS: {}", e))?
        .json::<Jwk>()
        .await
        .map_err(|e| format!("Failed to parse JWKS: {}", e))?;

    // 2. Decode the token's header to find the Key ID (`kid`)
    let header = decode_header(token_str).map_err(|e| format!("Invalid token header: {}", e))?;

    let kid = header
        .kid
        .ok_or_else(|| "Token header missing 'kid'".to_string())?;

    // 3. Find the matching public key from the JWKS
    let matching_key = jwks
        .keys
        .iter()
        .find(|key| key.kid == kid)
        .ok_or_else(|| "No matching public key found".to_string())?;

    // 4. Create a decoding key from the public key's components (n, e)
    let decoding_key = DecodingKey::from_rsa_components(&matching_key.n, &matching_key.e)
        .map_err(|e| format!("Failed to create decoding key: {}", e))?;

    // 5. Set up validation rules
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]); // Check 'aud'
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]); // Check 'iss'

    // 6. Decode and validate the token
    // This function checks the signature, expiration, issuer, and audience.
    let token_data = decode::<GoogleClaims>(token_str, &decoding_key, &validation)
        .map_err(|e| format!("Token validation failed: {}", e))?;

    Ok(token_data.claims)
}
