use aws_sigv4::http_request::{SignableRequest, SigningParams, SigningSettings, sign as aws_sign};
use http_02::Request;
use ring::digest;
use std::collections::BTreeMap;
use std::time::SystemTime;

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
    pub time: SystemTime,
    pub body: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct SignedRequest {
    pub payload_hash: String,
    pub signature: String,
    pub authorization: String,
    pub headers: BTreeMap<String, String>,
}

pub fn sign(req: SignRequest<'_>, extra_headers: &[(&str, &str)]) -> Result<SignedRequest, String> {
    let payload_hash = hex_sha256(req.body);
    let url = if req.query.is_empty() {
        format!("https://{}{}", req.host, req.uri)
    } else {
        format!("https://{}{}?{}", req.host, req.uri, req.query)
    };
    let mut request = Request::builder()
        .method(req.method)
        .uri(url)
        .header("host", req.host)
        .header("x-amz-content-sha256", &payload_hash)
        .body(req.body)
        .map_err(|error| format!("AWS signing request construction failed: {error}"))?;
    for (key, value) in extra_headers {
        let key = key.to_ascii_lowercase();
        if key == "authorization" || key == "host" || key.starts_with("x-amz-") {
            continue;
        }
        let name = http_02::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|_| format!("Invalid AWS signing header name `{key}`"))?;
        let value = http_02::header::HeaderValue::from_str(value.trim())
            .map_err(|_| format!("Invalid AWS signing header value for `{key}`"))?;
        request.headers_mut().insert(name, value);
    }

    let settings = SigningSettings::default();
    let mut params = SigningParams::builder()
        .access_key(req.access_key)
        .secret_key(req.secret_key)
        .region(req.region)
        .service_name(req.service)
        .time(req.time)
        .settings(settings);
    if let Some(token) = req.session_token {
        params = params.security_token(token);
    }
    let params = params
        .build()
        .map_err(|error| format!("AWS signing parameters are invalid: {error}"))?;
    let (instructions, signature) = aws_sign(SignableRequest::from(&request), &params)
        .map_err(|error| format!("AWS SigV4 signing failed: {error}"))?
        .into_parts();
    instructions.apply_to_request(&mut request);

    let headers = request
        .headers()
        .iter()
        .map(|(name, value)| {
            value
                .to_str()
                .map(|value| (name.as_str().to_string(), value.to_string()))
                .map_err(|_| format!("AWS signer produced a non-text header `{name}`"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let authorization = headers
        .get("authorization")
        .cloned()
        .ok_or_else(|| "AWS signer did not produce an authorization header".to_string())?;
    Ok(SignedRequest {
        payload_hash,
        signature,
        authorization,
        headers,
    })
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
