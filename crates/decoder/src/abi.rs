//! ABI args decoder — `(signature, input)` → typed Solidity values (S11.1).
//!
//! Built on `alloy::dyn_abi` — runtime ABI parsing keyed by the canonical
//! Solidity signature string we already store in `function_signature.signature`
//! (S11). The first 4 bytes of `input` are the selector that S11 already
//! looked up; this module focuses on the **remaining bytes**, decoded against
//! the signature's type sequence and lowered to JSON for the API response.
//!
//! ## Why alloy and not a hand-rolled decoder
//!
//! `alloy` is already in the workspace (`alloy = "1"`, features `["full"]`)
//! for the indexer's RPC client. `alloy::dyn_abi::DynSolType::parse_seq`
//! covers the full Solidity type system — addresses, sized integers, dynamic
//! bytes, fixed arrays, dynamic arrays, **nested tuples** (which our seeded
//! `exactInputSingle((address,address,uint24,...))` needs). Reimplementing
//! that surface for 17 seeded selectors would be larger than the slice itself
//! and would carry corner-case risk every time we extend the seed (D025).
//!
//! ## What `null` means
//!
//! A `null` `args` (returned as the `Option::None` from a caller) is the
//! catch-all "decoding did not yield a clean value" signal — the selector
//! lookup may have succeeded (so `name` + `signature` are still reported),
//! but the input may be missing, too short, malformed, or shaped differently
//! than the seeded signature claims. We don't fail the whole response on a
//! decode miss; `name` + `signature` is still useful diagnostic data
//! (D027).

use alloy::dyn_abi::{DynSolType, DynSolValue, Error as DynAbiError};
use alloy::primitives::hex;
use serde::Serialize;
use serde_json::Value;

/// Selector occupies the first 4 bytes of `input`. The decoder always strips
/// those before passing the rest to `abi_decode_params`.
const SELECTOR_BYTES: usize = 4;

/// Errors returned by [`decode_args`]. The API handler maps every variant to
/// `args: None` (S11.1 / D027 — decode misses don't collapse the surrounding
/// `DecodedFunction` object). The variant carries enough context to log a
/// useful trace line; nothing leaks to the response body.
#[derive(Debug, thiserror::Error)]
pub enum AbiDecodeError {
    /// `signature` didn't contain a `(...)` parameter list. Most callers
    /// won't hit this — our seed is curated — but the API takes `&str` so
    /// this guards the boundary anyway.
    #[error("invalid signature (missing parameter list): {0}")]
    InvalidSignature(String),

    /// `input_hex` couldn't be hex-decoded. Includes the underlying parse
    /// error for diagnostics.
    #[error("invalid hex input: {0}")]
    InvalidHex(String),

    /// `input_hex` decoded to fewer than 4 bytes — can't even host a selector,
    /// let alone arguments.
    #[error("input shorter than selector (4 bytes)")]
    InputTooShort,

    /// `alloy::dyn_abi` rejected the args bytes against the type sequence.
    /// Most often: length mismatch, malformed dynamic offset, etc.
    #[error("abi decode failed: {0}")]
    Decode(String),
}

/// A single decoded argument — Solidity type plus its JSON-friendly value.
///
/// Field order matches the function signature (D026 — `Vec` for order
/// preservation). `name` is intentionally absent because the seeded
/// signatures are *anonymous* (e.g. `transfer(address,uint256)`); adding a
/// field that is *always* `null` would be misleading (silent default
/// rejection — D004/D014).
///
/// `value` representations (lowering rules):
/// - `address` → `"0x" + 40-hex` (lowercased)
/// - `uint{N}` / `int{N}` → decimal string (precision-safe across JS / Python)
/// - `bool` → JSON boolean
/// - `bytes` / `bytesN` → `"0x" + hex`
/// - `string` → JSON string
/// - tuple / fixed-array / dynamic array → JSON array (recursive)
///
/// Numbers go to strings on purpose — JSON's `number` only safely holds
/// integers up to 2^53; `uint256` would silently lose precision in any
/// JS/TS client otherwise.
#[derive(Debug, Clone, Serialize)]
pub struct DecodedArg {
    /// Solidity type string verbatim from the signature parameter list
    /// (e.g. `"address"`, `"uint256"`, `"(address,uint256)"`).
    #[serde(rename = "type")]
    pub ty: String,
    /// JSON-friendly value (see lowering rules above).
    pub value: Value,
}

/// Decode the argument bytes of a transaction or call against its signature.
///
/// `signature` is the canonical Solidity signature string we already store in
/// `function_signature.signature` (e.g. `"transfer(address,uint256)"`).
/// `input_hex` is the full call data including the 4-byte selector
/// (`"0xa9059cbb..."`). Both `0x`-prefixed and bare hex are accepted; case
/// is irrelevant.
pub fn decode_args(signature: &str, input_hex: &str) -> Result<Vec<DecodedArg>, AbiDecodeError> {
    let types_str = extract_param_list(signature)
        .ok_or_else(|| AbiDecodeError::InvalidSignature(signature.to_string()))?;

    let bytes = hex::decode(input_hex.strip_prefix("0x").unwrap_or(input_hex))
        .map_err(|e| AbiDecodeError::InvalidHex(e.to_string()))?;

    if bytes.len() < SELECTOR_BYTES {
        return Err(AbiDecodeError::InputTooShort);
    }
    let args_bytes = &bytes[SELECTOR_BYTES..];

    // alloy exposes `DynSolType::parse` for single types only — to decode the
    // full param list we wrap it as a tuple and pull the values back out of
    // the decoded `DynSolValue::Tuple`. The empty arg case (`""` → `"()"`)
    // round-trips to an empty tuple, so callers stay uniform.
    let wrapped = format!("({types_str})");
    let seq_ty: DynSolType = wrapped
        .parse::<DynSolType>()
        .map_err(|e: DynAbiError| AbiDecodeError::Decode(format!("signature parse: {e}")))?;

    let decoded = seq_ty
        .abi_decode_params(args_bytes)
        .map_err(|e: DynAbiError| AbiDecodeError::Decode(e.to_string()))?;
    let values: Vec<DynSolValue> = match decoded {
        DynSolValue::Tuple(vs) => vs,
        // Shouldn't reach — we always wrap as a tuple — but unwrapping
        // defensively keeps the function total.
        other => vec![other],
    };

    // The flat list of top-level types comes back as a tuple of one value per
    // parameter. Walk the original parameter list (already validated above)
    // so the `type` strings we report match the signature character-for-
    // character — useful when downstream cares about, e.g.,
    // `uint24` vs the default `uint256`.
    let top_types = split_top_level(types_str);

    Ok(values
        .into_iter()
        .zip(top_types)
        .map(|(v, ty)| DecodedArg {
            ty: ty.to_string(),
            value: dynsol_to_json(&v),
        })
        .collect())
}

/// Pull the parameter list out of `"name(types)"`. Returns `Some("types")`
/// for any well-formed signature, including the empty-arg case `"name()"`
/// (returns `Some("")`).
fn extract_param_list(signature: &str) -> Option<&str> {
    let open = signature.find('(')?;
    let close = signature.rfind(')')?;
    if close <= open {
        return None;
    }
    Some(&signature[open + 1..close])
}

/// Split a top-level type list on commas, **ignoring commas inside nested
/// `(...)` tuple types**. e.g.
/// `"address,(address,uint24,uint256),bytes"` → `["address",
/// "(address,uint24,uint256)", "bytes"]`.
///
/// Returns an empty `Vec` for the empty-arg case (`""`), keeping callers
/// uniform.
fn split_top_level(types_str: &str) -> Vec<&str> {
    if types_str.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, b) in types_str.bytes().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                out.push(types_str[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(types_str[start..].trim());
    out
}

/// Lower a single `DynSolValue` to a JSON-friendly representation per the
/// rules documented on [`DecodedArg`].
fn dynsol_to_json(v: &DynSolValue) -> Value {
    match v {
        DynSolValue::Address(a) => Value::String(format!("0x{}", hex::encode(a.as_slice()))),
        DynSolValue::Uint(n, _bits) => Value::String(n.to_string()),
        DynSolValue::Int(n, _bits) => Value::String(n.to_string()),
        DynSolValue::Bool(b) => Value::Bool(*b),
        DynSolValue::Bytes(b) => Value::String(format!("0x{}", hex::encode(b))),
        DynSolValue::FixedBytes(w, _) => Value::String(format!("0x{}", hex::encode(w.as_slice()))),
        DynSolValue::String(s) => Value::String(s.clone()),
        DynSolValue::Array(items) | DynSolValue::FixedArray(items) | DynSolValue::Tuple(items) => {
            Value::Array(items.iter().map(dynsol_to_json).collect())
        }
        // `function`, `enum`, and the custom-struct variants don't show up in
        // our seed (and `parse_seq` wouldn't produce them from canonical
        // signature strings anyway). Map to `null` defensively; the caller
        // will treat the whole `args` as `None` if any element here lands at
        // `null` via the decode-error path, but a single `null` element is
        // still surfaced honestly to the response.
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};
    use alloy::sol_types::SolValue;

    /// Encode a `(Address, U256)` as the calldata body for
    /// `transfer(address,uint256)` — selector prepended for `decode_args`.
    fn encode_transfer_calldata(to: Address, amount: U256) -> String {
        let body = hex::encode((to, amount).abi_encode_params());
        format!("0xa9059cbb{body}")
    }

    #[test]
    fn decode_transfer_address_uint256() {
        let to = Address::repeat_byte(0xab);
        let amount = U256::from(1_000_000_000_000_000_000u64);
        let calldata = encode_transfer_calldata(to, amount);

        let args = decode_args("transfer(address,uint256)", &calldata).expect("decode ok");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].ty, "address");
        assert_eq!(
            args[0].value,
            Value::String(format!("0x{}", "ab".repeat(20)))
        );
        assert_eq!(args[1].ty, "uint256");
        assert_eq!(args[1].value, Value::String(amount.to_string()));
    }

    #[test]
    fn decode_approve_address_uint256() {
        // approve(spender, amount) has the same shape as transfer; reuse the encoder.
        let spender = Address::repeat_byte(0xcd);
        let amount = U256::from(42u64);
        let body = hex::encode((spender, amount).abi_encode_params());
        let calldata = format!("0x095ea7b3{body}");

        let args = decode_args("approve(address,uint256)", &calldata).expect("decode ok");
        assert_eq!(args.len(), 2);
        assert_eq!(args[1].value, Value::String("42".to_string()));
    }

    #[test]
    fn decode_exact_input_single_tuple() {
        // Uniswap V3 SwapRouter.exactInputSingle — encodes a struct as the
        // single top-level argument. The signature is:
        //   exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))
        let token_in = Address::repeat_byte(0x01);
        let token_out = Address::repeat_byte(0x02);
        let fee: u32 = 3000;
        let recipient = Address::repeat_byte(0x03);
        let deadline = U256::from(1_700_000_000u64);
        let amount_in = U256::from(1_000_000u64);
        let amount_out_min = U256::from(990_000u64);
        let sqrt_price_limit = U256::from(0u64);

        // Encode the tuple as a single top-level param via abi_encode_params.
        let tuple = (
            token_in,
            token_out,
            fee,
            recipient,
            deadline,
            amount_in,
            amount_out_min,
            sqrt_price_limit,
        );
        // Wrap the tuple inside another singleton tuple — the signature has
        // *one* arg whose type *is* a tuple, so encoded the same way as a
        // single-element param list.
        let body = hex::encode((tuple,).abi_encode_params());
        let calldata = format!("0x414bf389{body}");

        let args = decode_args(
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))",
            &calldata,
        )
        .expect("tuple decode ok");
        assert_eq!(args.len(), 1);
        assert_eq!(
            args[0].ty,
            "(address,address,uint24,address,uint256,uint256,uint256,uint160)"
        );
        let arr = args[0].value.as_array().expect("nested tuple → array");
        assert_eq!(arr.len(), 8);
        assert_eq!(arr[2], Value::String("3000".to_string())); // fee
        assert_eq!(arr[5], Value::String("1000000".to_string())); // amount_in
    }

    #[test]
    fn decode_invalid_hex_returns_err() {
        let result = decode_args("transfer(address,uint256)", "0xnothex");
        assert!(matches!(result, Err(AbiDecodeError::InvalidHex(_))));
    }

    #[test]
    fn decode_invalid_signature_returns_err() {
        let result = decode_args("not a function", "0xa9059cbb");
        assert!(matches!(result, Err(AbiDecodeError::InvalidSignature(_))));
    }

    #[test]
    fn decode_input_too_short_returns_err() {
        // Only 3 hex bytes — selector needs 4.
        let result = decode_args("transfer(address,uint256)", "0xabcdef");
        assert!(matches!(result, Err(AbiDecodeError::InputTooShort)));
    }

    #[test]
    fn decode_mismatched_args_returns_err() {
        // Selector + nothing else → decode against `(address,uint256)` fails.
        let result = decode_args("transfer(address,uint256)", "0xa9059cbb");
        assert!(matches!(result, Err(AbiDecodeError::Decode(_))));
    }

    #[test]
    fn split_top_level_handles_nested_tuples() {
        assert_eq!(
            split_top_level("address,(address,uint24,uint256),bytes"),
            vec!["address", "(address,uint24,uint256)", "bytes"]
        );
        assert_eq!(
            split_top_level("address,uint256"),
            vec!["address", "uint256"]
        );
        assert_eq!(split_top_level(""), Vec::<&str>::new());
    }

    #[test]
    fn extract_param_list_handles_empty_args() {
        assert_eq!(extract_param_list("name()"), Some(""));
        assert_eq!(
            extract_param_list("transfer(address,uint256)"),
            Some("address,uint256")
        );
        assert_eq!(extract_param_list("no parens here"), None);
    }
}
