//! Operating-system entropy for native supervisor attempt identifiers.

/// Generates the host-random bytes bound into one supervisor attempt.
///
/// Callers must still reject zero and previously used values before dispatch.
/// This function supplies entropy, not request authentication or admission.
///
/// # Errors
///
/// Returns the operating-system randomness failure without a fallback value.
pub fn random_attempt_nonce() -> Result<[u8; 32], getrandom::Error> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes)
}
