#[cfg(test)]
mod tests {
    use super::{
        CodingAgentProductEventFamily, CodingAgentProductEventKind,
        CodingAgentProductEventTerminalStatus, CodingAgentProductEventDurability,
    };

    #[test]
    fn typed_contract_has_stable_names_and_independent_metadata() {
        assert_eq!(CodingAgentProductEventFamily::Session.as_str(), "session");
        assert_eq!(CodingAgentProductEventTerminalStatus::Completed.as_str(), "completed");
        assert_eq!(
            serde_json::to_string(&CodingAgentProductEventDurability::LiveOnly).unwrap(),
            "\"live_only\""
        );
        let _typed_kind: Option<CodingAgentProductEventKind> = None;
    }
}
