use ring::{digest, hmac};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct SignRequest<'a> {
    pub method: &'a str,
    pub uri: &'a str,
    pub query: &'a str,
    pub host: &'a str,
    pub region: &'a str,
    pub service: &'a str,
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub session_token: Option<&'a str>,
    pub amz_date: &'a str,
    pub date: &'a str,
    pub body: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct SignedRequest {
    pub payload_hash: String,
    pub canonical_request: String,
    pub string_to_sign: String,
    pub signature: String,
    pub authorization: String,
    pub headers: BTreeMap<String, String>,
}

pub fn sign(req: SignRequest<'_>, extra_headers: &[(&str, &str)]) -> SignedRequest {
    let payload_hash = hex_sha256(req.body);
    let mut headers = BTreeMap::new();
    headers.insert("host".to_string(), req.host.to_string());
    headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
    headers.insert("x-amz-date".to_string(), req.amz_date.to_string());
    if let Some(token) = req.session_token {
        headers.insert("x-amz-security-token".to_string(), token.to_string());
    }
    for (key, value) in extra_headers {
        let key = key.to_ascii_lowercase();
        if key == "authorization" || key == "host" || key.starts_with("x-amz-") {
            continue;
        }
        headers.insert(key, value.trim().to_string());
    }

    let canonical_headers = headers
        .iter()
        .map(|(key, value)| format!("{}:{}\n", key, normalize_header_value(value)))
        .collect::<String>();
    let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        req.method, req.uri, req.query, canonical_headers, signed_headers, payload_hash
    );
    let canonical_hash = hex_sha256(canonical_request.as_bytes());
    let scope = format!("{}/{}/{}/aws4_request", req.date, req.region, req.service);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        req.amz_date, scope, canonical_hash
    );
    let signing_key = signing_key(req.secret_key, req.date, req.region, req.service);
    let signature = hex_hmac(&signing_key, string_to_sign.as_bytes());
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        req.access_key, scope, signed_headers, signature
    );

    headers.insert("authorization".to_string(), authorization.clone());
    SignedRequest {
        payload_hash,
        canonical_request,
        string_to_sign,
        signature,
        authorization,
        headers,
    }
}

fn signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_bytes(format!("AWS4{}", secret_key).as_bytes(), date.as_bytes());
    let k_region = hmac_bytes(&k_date, region.as_bytes());
    let k_service = hmac_bytes(&k_region, service.as_bytes());
    hmac_bytes(&k_service, b"aws4_request")
}

fn hmac_bytes(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

fn hex_hmac(key: &[u8], data: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hex(hmac::sign(&key, data).as_ref())
}

fn hex_sha256(data: &[u8]) -> String {
    hex(digest::digest(&digest::SHA256, data).as_ref())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn normalize_header_value(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
