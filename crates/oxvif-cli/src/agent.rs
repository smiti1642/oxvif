use crate::{AgentGuide, SCHEMA_VERSION};

pub const GUIDE_VERSION: &str = "8";

pub fn guide() -> AgentGuide {
    AgentGuide {
        guide_version: GUIDE_VERSION,
        cli_version: env!("CARGO_PKG_VERSION"),
        schema_version: SCHEMA_VERSION,
        rules: vec![
            "manage is a terminal-only adapter, not an automation endpoint. Use the corresponding list, media.profiles, diagnose, media.snapshot-save and config export/diff commands with explicit selectors.",
            "Diagnosis assessment.primary_issue describes the first observed failure, not a proven root cause; inspect additional_issues, blocked_checks and limitations. Profile video fields are device-reported configuration, not measured playback/FPS; null means unavailable and details_status explains missing metadata.",
            "Diagnose profile_selection retains error_code PROFILE_SELECTION_REQUIRED for compatibility; inspect data.reason_code to distinguish missing selection, unknown token, query failure, no profiles, cancellation or interaction failure. data.candidates contains name/token pairs. Do not substitute an unknown explicit token automatically.",
            "Diagnose stages may include not_tested_reason: prerequisite_failed or not_implemented. Inspect summary counts and selected_profile; complete=true still does not establish video playback.",
            "For diagnose/config.export/config.diff, exit 20 may retain a device_diagnostic report: inspect failed stages, complete and incomparable_sections, not only error.code.",
            "A diagnose report never verifies RTSP transport or video decoding. Snapshot downloads validate signatures only; config exports are not restorable backups.",
            "snapshot --save and config export write sensitive local files and never overwrite; use one explicit device and an approved destination. config.diff differences alone exit 0; inspect matches and changes.",
            "Inspect a command with `oxvif describe <command> --output json` before invoking it.",
            "Use structured output and --non-interactive for automation.",
            "Select a device explicitly; never depend on the ambient current device.",
            "Treat global device IDs as canonical and group/local-alias values as selectors.",
            "Check ok, schema_version, warnings, error.code, and retryable on every result.",
            "Do not invoke write or dangerous operations without explicit authorization.",
            "Use plan/apply when an operation exposes that workflow.",
            "For device import, apply only the fingerprint returned by a freshly reviewed plan.",
            "For fleet work, use exactly one explicit --group or --view selector and keep --jobs at 64 or below.",
            "Treat fleet exit 6 as partial success: inspect every item and the final JSONL summary before retrying failures.",
            "Use --clock-sync auto unless the operator explicitly requires always or never; clock sync reads device time and never changes it.",
            "For HTTPS devices using a private trust anchor, pass operator-approved --ca-certificate PEM files; never disable certificate or hostname verification.",
        ],
        recommended_workflow: vec![
            "Run `oxvif agent guide --output json` and verify its schema version.",
            "Run `oxvif describe --output json` to discover implemented commands.",
            "Select the target explicitly and execute one typed command.",
            "Persist canonical device IDs returned in result metadata.",
            "Use registration=saved or registration=unregistered filters to separate known and new discovery records; use --query for the same cross-field search as the human discovery browser; inspect each record's registration_status and registered_device_id.",
            "Enrich and filter a discovery snapshot, review its import plan, then apply that exact fingerprint.",
            "Run read-only inspection against a Group/View with JSONL and consume device lines followed by the aggregate summary.",
            "Retry only when the structured error says retryable=true.",
        ],
        security_requirements: vec![
            "Never place passwords in command arguments, logs, prompts, or registry files.",
            "Use password stdin, environment injection, or a native credential profile.",
            "Never copy URI-embedded credentials into output or persistent state.",
            "Do not infer authorization for device writes from read-only access.",
        ],
    }
}

pub fn prompt() -> String {
    format!(
        "You are operating oxvif CLI version {} with structured schema {}.\n\
Before invoking a command:\n\
1. Run `oxvif describe <command> --output json`.\n\
2. Use `--output json` and `--non-interactive`.\n\
3. Select devices explicitly; never depend on `current`.\n\
4. Never pass passwords in arguments or write them to logs.\n\
5. Check `ok`, `schema_version`, `warnings`, `error.code`, and `retryable`.\n\
6. Do not execute write or dangerous commands without explicit authorization.\n\
7. Use plan/apply when supported.",
        env!("CARGO_PKG_VERSION"),
        SCHEMA_VERSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_guide_tracks_the_stdout_schema() {
        let guide = guide();
        assert_eq!(guide.schema_version, SCHEMA_VERSION);
        assert!(!guide.rules.is_empty());
        assert!(prompt().contains("--non-interactive"));
    }
}
