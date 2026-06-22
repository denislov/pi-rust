use super::sigv4;
use crate::types::StreamOptions;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
}

pub fn auth_headers(
    url: &str,
    region: &str,
    body: &[u8],
    opts: &Option<StreamOptions>,
) -> Result<BTreeMap<String, String>, String> {
    if let Some(token) = opts
        .as_ref()
        .and_then(|o| o.bedrock_bearer_token.clone())
        .or_else(|| std::env::var("AWS_BEARER_TOKEN_BEDROCK").ok())
        .filter(|token| !token.trim().is_empty())
    {
        return Ok(BTreeMap::from([(
            "authorization".into(),
            format!("Bearer {}", token),
        )]));
    }

    let credentials = resolve_credentials(None)?;
    let (host, uri, query) = parse_url_for_signing(url)?;
    let (date, amz_date) = super::datetime::current_aws_dates();
    let signed = sigv4::sign(
        sigv4::SignRequest {
            method: "POST",
            uri: &uri,
            query: &query,
            host: &host,
            region,
            service: "bedrock",
            access_key: &credentials.access_key,
            secret_key: &credentials.secret_key,
            session_token: credentials.session_token.as_deref(),
            amz_date: &amz_date,
            date: &date,
            body,
        },
        &[],
    );
    Ok(signed.headers)
}

pub fn resolve_credentials(explicit: Option<(String, String)>) -> Result<AwsCredentials, String> {
    if let Some((access_key, secret_key)) = explicit {
        return Ok(AwsCredentials {
            access_key,
            secret_key,
            session_token: None,
        });
    }
    let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
    match (access_key, secret_key) {
        (Some(access_key), Some(secret_key))
            if !access_key.is_empty() && !secret_key.is_empty() =>
        {
            Ok(AwsCredentials {
                access_key,
                secret_key,
                session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            })
        }
        _ => Err("No AWS credentials found. Set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, AWS_BEARER_TOKEN_BEDROCK, or pass a Bedrock bearer token.".into()),
    }
}

pub fn parse_url_for_signing(url: &str) -> Result<(String, String, String), String> {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return Err(format!("Invalid Bedrock URL: {}", url));
    };
    let (host, path_and_query) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{}", path)),
        None => (rest, "/".into()),
    };
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (path_and_query, String::new()),
    };
    Ok((host.to_string(), path, query))
}

pub fn region_from_endpoint(base_url: &str) -> Option<String> {
    let host = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))?
        .split('/')
        .next()?;
    let mut parts = host.split('.');
    if parts.next()? != "bedrock-runtime" {
        return None;
    }
    parts.next().map(str::to_string)
}
