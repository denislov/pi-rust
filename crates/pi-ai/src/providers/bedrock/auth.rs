use super::sigv4;
use crate::protocol::StreamOptions;
use aws_credential_types::provider::ProvideCredentials;
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
        .filter(|token| !token.trim().is_empty())
    {
        return Ok(BTreeMap::from([(
            "authorization".into(),
            format!("Bearer {}", token),
        )]));
    }

    let credentials = resolve_explicit_credentials_from_options(opts)?
        .ok_or_else(|| "No explicit AWS credentials found".to_string())?;
    let (host, uri, query) = parse_url_for_signing(url)?;
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
            time: std::time::SystemTime::now(),
            body,
        },
        &[],
    )?;
    Ok(signed.headers)
}

pub fn resolve_explicit_credentials_from_options(
    opts: &Option<StreamOptions>,
) -> Result<Option<AwsCredentials>, String> {
    let Some(opts) = opts.as_ref() else {
        return Ok(None);
    };
    match (
        opts.bedrock_access_key_id.clone(),
        opts.bedrock_secret_access_key.clone(),
    ) {
        (Some(access_key), Some(secret_key))
            if !access_key.trim().is_empty() && !secret_key.trim().is_empty() =>
        {
            Ok(Some(AwsCredentials {
                access_key,
                secret_key,
                session_token: opts.bedrock_session_token.clone(),
            }))
        }
        (None, None) => Ok(None),
        _ => Err(
            "Bedrock explicit credentials require both access key ID and secret access key".into(),
        ),
    }
}

pub async fn resolve_credentials_from_chain(
    profile: Option<&str>,
) -> Result<AwsCredentials, String> {
    let mut loader = aws_config::from_env();
    if let Some(profile) = profile.filter(|profile| !profile.trim().is_empty()) {
        loader = loader.profile_name(profile);
    }
    let config = loader.load().await;
    let provider = config
        .credentials_provider()
        .ok_or_else(|| "AWS credential provider chain is unavailable".to_string())?;
    let credentials = provider.provide_credentials().await.map_err(|_| {
        "AWS credential provider chain did not return usable credentials".to_string()
    })?;
    Ok(AwsCredentials {
        access_key: credentials.access_key_id().to_string(),
        secret_key: credentials.secret_access_key().to_string(),
        session_token: credentials.session_token().map(str::to_string),
    })
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
