use crate::session::{SessionError, SessionErrorCode, generate_entry_id};
use serde_json::{Number, Value};
use std::collections::HashSet;

const CURRENT_SESSION_VERSION: u64 = 3;

pub(super) fn migrate_session_values(values: &mut [Value]) -> Result<bool, SessionError> {
    let header = values
        .first()
        .ok_or_else(|| SessionError::new(SessionErrorCode::InvalidSession, "empty session file"))?;

    let header_object = header.as_object().ok_or_else(|| {
        SessionError::new(
            SessionErrorCode::InvalidSession,
            "first line is not a valid session header",
        )
    })?;

    if header_object.get("type").and_then(Value::as_str) != Some("session") {
        return Err(SessionError::new(
            SessionErrorCode::InvalidSession,
            "first line is not a valid session header: missing type=session",
        ));
    }

    let version = match header_object.get("version") {
        Some(value) => value.as_u64().ok_or_else(|| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                "first line is not a valid session header: invalid version",
            )
        })?,
        None => 1,
    };

    if version > CURRENT_SESSION_VERSION {
        return Err(SessionError::new(
            SessionErrorCode::InvalidSession,
            format!("unsupported session version: {}", version),
        ));
    }

    let mut migrated = false;
    if version < 2 {
        migrate_v1_to_v2(values)?;
        migrated = true;
    }
    if version < 3 {
        migrate_v2_to_v3(values);
        migrated = true;
    }

    Ok(migrated)
}

fn migrate_v1_to_v2(values: &mut [Value]) -> Result<(), SessionError> {
    let mut ids = HashSet::new();
    let mut previous_id: Option<String> = None;

    for index in 0..values.len() {
        let mut first_kept_entry_index = None;

        {
            let entry = values[index].as_object_mut().ok_or_else(|| {
                SessionError::new(
                    SessionErrorCode::InvalidEntry,
                    format!("entry at line {} is not an object", index + 1),
                )
            })?;
            let entry_type = entry
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            if entry_type == "session" {
                entry.insert("version".into(), Value::Number(Number::from(2)));
                continue;
            }

            let entry_id = generate_entry_id(&ids);
            ids.insert(entry_id.clone());

            entry.insert("id".into(), Value::String(entry_id.clone()));
            entry.insert(
                "parentId".into(),
                previous_id
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            previous_id = Some(entry_id);

            if entry_type == "compaction" {
                first_kept_entry_index = entry
                    .get("firstKeptEntryIndex")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok());
                entry.remove("firstKeptEntryIndex");
            }
        }

        if let Some(target_index) = first_kept_entry_index {
            let first_kept_entry_id = values
                .get(target_index)
                .and_then(Value::as_object)
                .and_then(|entry| {
                    if entry.get("type").and_then(Value::as_str) == Some("session") {
                        None
                    } else {
                        entry.get("id").and_then(Value::as_str).map(str::to_string)
                    }
                });

            if let Some(first_kept_entry_id) = first_kept_entry_id {
                if let Some(entry) = values[index].as_object_mut() {
                    entry.insert(
                        "firstKeptEntryId".into(),
                        Value::String(first_kept_entry_id),
                    );
                }
            }
        }
    }

    Ok(())
}

fn migrate_v2_to_v3(values: &mut [Value]) {
    for value in values {
        let Some(entry) = value.as_object_mut() else {
            continue;
        };
        let entry_type = entry.get("type").and_then(Value::as_str);

        if entry_type == Some("session") {
            entry.insert("version".into(), Value::Number(Number::from(3)));
            continue;
        }

        if entry_type == Some("message") {
            let Some(message) = entry.get_mut("message").and_then(Value::as_object_mut) else {
                continue;
            };
            if message.get("role").and_then(Value::as_str) == Some("hookMessage") {
                message.insert("role".into(), Value::String("custom".into()));
            }
        }
    }
}
