#!/usr/bin/env bash
#
# validate-audit.sh - Phase 1 audit artifact structural, evidence, and traceability validator.
#
# Modes:
#   --schema-only    Validate structure: headings, columns, 15 variants, taxonomy, IDs, source count
#   --evidence-only  Validate evidence: no placeholders, evidence IDs exist, command ledger complete
#   (default)        Final mode: evidence + Audit Status: final, traceability, finding categories, zero blockers
#
# This script parses Markdown as untrusted text data only. It never evals, sources, or executes
# any content from the audit file or command ledger. It uses only standard POSIX/Bash tools.
#
set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
AUDIT_FILE="$SCRIPT_DIR/01-AUDIT.md"
SOURCE_FILE="$REPO_ROOT/crates/pi-coding-agent/src/coding_session/public_operation.rs"

# Fixed exact public variant list (from CodingAgentOperation enum)
readonly VARIANTS=(
  Prompt Compact BranchSummary SelfHealingEdit InvokeAgent InvokeTeam
  PluginLoad PluginCommand SetDefaultAgentProfile ApproveDelegation
  RejectDelegation ForkSession SwitchActiveLeaf ExportCurrent ExportCurrentHtml
)
readonly EXPECTED_VARIANT_COUNT=15

# Locked taxonomy values (D-10, D-12 through D-16)
readonly IMPL_VALUES='complete|partial|missing|not_applicable'
readonly VERIFY_VALUES='passed|failed|blocked|not_run'
readonly DISP_VALUES='active|obsolete|deferred_stage_10|retained_compatibility'
readonly CONF_VALUES='high|medium|low'
readonly OBLIG_VALUES='blocking|required|hardening|informational'

# Known requirement ID pattern
readonly REQ_REGEX='^(AUDIT|FACADE|ADAPT|RPC|INTER|TEST|DELETE|GUARD|CLOSE)-[0-9]+$'

# Allowed finding target phases
readonly TARGET_PHASE_REGEX='^Phase [2-5]$'

# Required section headings (## level)
readonly REQUIRED_HEADINGS=(
  "Audit Contract"
  "Authority Order"
  "Evidence Index"
  "Command Ledger"
  "Operation Matrix"
  "Production Caller Inventory"
  "Test Caller Inventory"
  "Compatibility Inventory"
  "Authority Reconciliation"
  "Findings"
  "Requirement Traceability"
  "Validation Summary"
)

# Required Operation Matrix column headers
readonly OP_MATRIX_REQUIRED_COLS=(
  variant internal kind origin class dispatch outcome
  prod_callers test_callers impl verify disp conf evidence gaps blockers
)

# Required Findings table column headers
readonly FINDINGS_REQUIRED_COLS=(
  ID Obligation Disposition Description Evidence Requirements "Target Phase" Dependencies Confidence Gaps Blockers
)

# Required Command Ledger column headers
readonly LEDGER_REQUIRED_COLS=(
  "Evidence ID" "Command" "Date" "Exit Status" "Test Count" "Result"
)

# Required Compatibility Inventory column headers
readonly COMPAT_REQUIRED_COLS=(
  Method Visibility Deprecation "Matching Operation" "Prod Callers" "Test Callers" "Retention Reason" "Deletion Req" "Target Phase"
)

# Required Requirement Traceability column headers
readonly TRACEABILITY_REQUIRED_COLS=(
  Requirement Description Status Evidence Notes
)

# Placeholder patterns (rejected in evidence mode)
readonly PLACEHOLDER_PATTERNS='^(pending|TBD|-|none recorded|populated by|_)'

# ---------------------------------------------------------------------------
# Error tracking
# ---------------------------------------------------------------------------

ERRORS=()
WARNINGS=()

add_error()   { ERRORS+=("ERROR: $1"); }
add_warning() { WARNINGS+=("WARNING: $1"); }

# ---------------------------------------------------------------------------
# Mode parsing
# ---------------------------------------------------------------------------

MODE="final"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --schema-only)   MODE="schema";   shift ;;
    --evidence-only) MODE="evidence"; shift ;;
    --help|-h)
      echo "Usage: $0 [--schema-only|--evidence-only]"
      echo "  --schema-only    Validate structure only"
      echo "  --evidence-only  Validate evidence completeness"
      echo "  (default)        Full final validation"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------

# Check if a line is a Markdown table separator row (only |, -, :, spaces)
is_separator_row() {
  local line="$1"
  # Remove all |, -, :, spaces - if nothing left, it's a separator
  local stripped
  stripped="${line//|/}"
  stripped="${stripped//-/}"
  stripped="${stripped//:/}"
  stripped="${stripped// /}"
  stripped="${stripped//$'\t'/}"
  [[ -z "$stripped" ]]
}

# Check if a line is a Markdown table row (starts with |)
is_table_row() {
  local line="$1"
  [[ "$line" =~ ^[[:space:]]*\| ]]
}

# Extract a cell from a table row by 1-based field number (using | as delimiter)
# Field 1 is empty (before first |), field 2 is first cell, etc.
get_cell() {
  local row="$1" field_num="$2"
  echo "$row" | cut -d'|' -f"$field_num" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//'
}

# Count cells in a table row
count_cells() {
  local row="$1"
  # Count | characters, subtract 1 for leading, cells = pipes - 1
  local pipe_count
  pipe_count=$(echo "$row" | tr -cd '|' | wc -c)
  echo $((pipe_count - 1))
}

# Find the line number where a ## heading starts
find_heading_line() {
  local file="$1" heading="$2"
  grep -n "^## ${heading}$" "$file" 2>/dev/null | head -1 | cut -d: -f1
}

# Find the line number of the next ## heading after a given line
find_next_heading_line() {
  local file="$1" after_line="$2"
  awk -v s="$after_line" 'NR>s && /^## /{print NR; exit}' "$file"
}

# Extract all table rows (excluding separator rows) from a section
# Args: file, heading_name
# Outputs: header_row, then data rows (one per line)
extract_section_table() {
  local file="$1" heading="$2"
  local start_line next_line

  start_line=$(find_heading_line "$file" "$heading")
  if [[ -z "$start_line" ]]; then
    return 1
  fi

  next_line=$(find_next_heading_line "$file" "$start_line")
  if [[ -z "$next_line" ]]; then
    next_line=$(wc -l < "$file")
    next_line=$((next_line + 1))
  fi

  # Extract lines that are table rows, skip separator rows
  awk -v s="$start_line" -v e="$next_line" '
    NR>s && NR<e && /^\|/ {
      # Check if separator row (only |, -, :, spaces)
      stripped = $0
      gsub(/[|\-: \t]/, "", stripped)
      if (stripped == "") next
      print
    }
  ' "$file"
}

# Extract only the header row (first table row) from a section
extract_table_header() {
  local file="$1" heading="$2"
  extract_section_table "$file" "$heading" 2>/dev/null | head -1
}

# Extract data rows (all table rows except the first/header) from a section
extract_table_data_rows() {
  local file="$1" heading="$2"
  extract_section_table "$file" "$heading" 2>/dev/null | tail -n +2
}

# Check if a value is a placeholder
is_placeholder() {
  local value="$1"
  [[ -z "$value" ]] && return 0
  [[ "$value" =~ ^[[:space:]]*$ ]] && return 0
  [[ "$value" =~ ^_\( ]] && return 0
  [[ "$value" == "-" ]] && return 0
  [[ "$value" == "pending" ]] && return 0
  [[ "$value" == "TBD" ]] && return 0
  [[ "$value" == "none" ]] && return 1  # 'none' is a valid explicit value, not a placeholder
  return 1
}

# Check if a value matches a pipe-delimited set of allowed values
matches_taxonomy() {
  local value="$1" allowed="$2"
  if [[ -z "$value" ]]; then
    return 0  # Empty is allowed in schema mode (caller decides)
  fi
  echo "$allowed" | grep -qE "^${value}$"
}

# ---------------------------------------------------------------------------
# Schema mode validations
# ---------------------------------------------------------------------------

validate_schema() {
  # 1. Audit file exists
  if [[ ! -f "$AUDIT_FILE" ]]; then
    add_error "Audit file not found: $AUDIT_FILE"
    return
  fi

  # 2. All required headings exist
  for heading in "${REQUIRED_HEADINGS[@]}"; do
    if [[ -z "$(find_heading_line "$AUDIT_FILE" "$heading")" ]]; then
      add_error "Missing required heading: ## $heading"
    fi
  done

  # 3. Audit Status line exists
  if ! grep -qi '^Audit Status:' "$AUDIT_FILE" 2>/dev/null; then
    add_error "Missing 'Audit Status:' line"
  fi

  # 4. Operation Matrix: header row has required columns
  local op_header
  op_header=$(extract_table_header "$AUDIT_FILE" "Operation Matrix")
  if [[ -z "$op_header" ]]; then
    add_error "Operation Matrix table not found"
  else
    local col_idx=2
    for col_name in "${OP_MATRIX_REQUIRED_COLS[@]}"; do
      if ! echo "$op_header" | grep -qi "| ${col_name} |"; then
        add_error "Operation Matrix missing required column: ${col_name}"
      fi
      col_idx=$((col_idx + 1))
    done
  fi

  # 5. Operation Matrix: exactly 15 data rows with exact variant names
  local op_data_rows
  op_data_rows=$(extract_table_data_rows "$AUDIT_FILE" "Operation Matrix")
  if [[ -z "$op_data_rows" ]]; then
    add_error "Operation Matrix has no data rows"
  else
    local row_count
    row_count=$(echo "$op_data_rows" | wc -l)
    if [[ "$row_count" -ne "$EXPECTED_VARIANT_COUNT" ]]; then
      add_error "Operation Matrix has $row_count data rows, expected $EXPECTED_VARIANT_COUNT"
    fi

    # Check each variant appears exactly once
    local variant
    for variant in "${VARIANTS[@]}"; do
      local match_count
      match_count=$(echo "$op_data_rows" | grep -cE "^\| ${variant} \|" || true)
      if [[ "$match_count" -eq 0 ]]; then
        add_error "Operation Matrix missing variant: ${variant}"
      elif [[ "$match_count" -gt 1 ]]; then
        add_error "Operation Matrix has duplicate variant: ${variant} (${match_count} occurrences)"
      fi
    done
  fi

  # 6. Taxonomy values (when non-empty) are valid
  # Operation Matrix columns: impl=field 11, verify=field 12, disp=field 13, conf=field 14
  local row
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue

    local impl_val verify_val disp_val conf_val
    impl_val=$(get_cell "$row" 11)
    verify_val=$(get_cell "$row" 12)
    disp_val=$(get_cell "$row" 13)
    conf_val=$(get_cell "$row" 14)

    if [[ -n "$impl_val" ]] && ! echo "$IMPL_VALUES" | grep -qE "^${impl_val}$"; then
      add_error "Invalid implementation value '$impl_val' in Operation Matrix row starting with: $(get_cell "$row" 2)"
    fi
    if [[ -n "$verify_val" ]] && ! echo "$VERIFY_VALUES" | grep -qE "^${verify_val}$"; then
      add_error "Invalid verification value '$verify_val' in Operation Matrix row starting with: $(get_cell "$row" 2)"
    fi
    if [[ -n "$disp_val" ]] && ! echo "$DISP_VALUES" | grep -qE "^${disp_val}$"; then
      add_error "Invalid disposition value '$disp_val' in Operation Matrix row starting with: $(get_cell "$row" 2)"
    fi
    if [[ -n "$conf_val" ]] && ! echo "$CONF_VALUES" | grep -qE "^${conf_val}$"; then
      add_error "Invalid confidence value '$conf_val' in Operation Matrix row starting with: $(get_cell "$row" 2)"
    fi

    # 7. Evidence gaps and blockers must be explicit 'none' when empty
    local gaps_val blockers_val
    gaps_val=$(get_cell "$row" 16)
    blockers_val=$(get_cell "$row" 17)
    if [[ -z "$gaps_val" ]] || [[ "$gaps_val" =~ ^[[:space:]]*$ ]]; then
      add_error "Evidence gaps must be explicit 'none' when empty (variant: $(get_cell "$row" 2))"
    fi
    if [[ -z "$blockers_val" ]] || [[ "$blockers_val" =~ ^[[:space:]]*$ ]]; then
      add_error "Blockers must be explicit 'none' when empty (variant: $(get_cell "$row" 2))"
    fi
  done <<< "$op_data_rows"

  # 8. Findings table: header has required columns
  local findings_header
  findings_header=$(extract_table_header "$AUDIT_FILE" "Findings")
  if [[ -n "$findings_header" ]]; then
    for col_name in "${FINDINGS_REQUIRED_COLS[@]}"; do
      if ! echo "$findings_header" | grep -qi "| ${col_name} |"; then
        add_error "Findings table missing required column: ${col_name}"
      fi
    done
  else
    add_error "Findings table not found"
  fi

  # 9. Findings: validate taxonomy and target phase in data rows (when non-placeholder)
  local findings_data
  findings_data=$(extract_table_data_rows "$AUDIT_FILE" "Findings")
  local finding_ids=()
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    # Skip placeholder rows
    local first_cell
    first_cell=$(get_cell "$row" 2)
    if is_placeholder "$first_cell"; then
      continue
    fi

    # Collect finding ID for uniqueness check
    finding_ids+=("$first_cell")

    # Validate obligation (field 3)
    local oblig_val
    oblig_val=$(get_cell "$row" 3)
    if [[ -n "$oblig_val" ]] && ! echo "$OBLIG_VALUES" | grep -qE "^${oblig_val}$"; then
      add_error "Invalid finding obligation '$oblig_val' for finding: $first_cell"
    fi

    # Validate disposition (field 4)
    local f_disp_val
    f_disp_val=$(get_cell "$row" 4)
    if [[ -n "$f_disp_val" ]] && ! echo "$DISP_VALUES" | grep -qE "^${f_disp_val}$"; then
      add_error "Invalid finding disposition '$f_disp_val' for finding: $first_cell"
    fi

    # Validate confidence (field 10)
    local f_conf_val
    f_conf_val=$(get_cell "$row" 10)
    if [[ -n "$f_conf_val" ]] && ! echo "$CONF_VALUES" | grep -qE "^${f_conf_val}$"; then
      add_error "Invalid finding confidence '$f_conf_val' for finding: $first_cell"
    fi

    # Validate target phase (field 8) - must be Phase 2-5
    local target_phase
    target_phase=$(get_cell "$row" 8)
    if [[ -n "$target_phase" ]] && ! [[ "$target_phase" =~ $TARGET_PHASE_REGEX ]]; then
      add_error "Invalid finding target phase '$target_phase' for finding: $first_cell (must be Phase 2-5)"
    fi

    # Validate requirement IDs (field 7) - comma-separated, each must match known prefix
    local req_field
    req_field=$(get_cell "$row" 7)
    if [[ -n "$req_field" ]] && [[ "$req_field" != "-" ]]; then
      IFS=',' read -ra req_arr <<< "$req_field"
      for req_id in "${req_arr[@]}"; do
        req_id=$(echo "$req_id" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        if [[ -n "$req_id" ]] && ! [[ "$req_id" =~ $REQ_REGEX ]]; then
          add_error "Invalid requirement ID '$req_id' in finding: $first_cell"
        fi
      done
    fi
  done <<< "$findings_data"

  # 10. Finding ID uniqueness (non-placeholder IDs only)
  if [[ ${#finding_ids[@]} -gt 1 ]]; then
    local sorted_ids
    sorted_ids=$(printf '%s\n' "${finding_ids[@]}" | sort)
    local dupes
    dupes=$(echo "$sorted_ids" | uniq -d)
    if [[ -n "$dupes" ]]; then
      add_error "Duplicate finding IDs: $dupes"
    fi
  fi

  # 11. Command Ledger: header has required columns
  local ledger_header
  ledger_header=$(extract_table_header "$AUDIT_FILE" "Command Ledger")
  if [[ -n "$ledger_header" ]]; then
    for col_name in "${LEDGER_REQUIRED_COLS[@]}"; do
      if ! echo "$ledger_header" | grep -qi "| ${col_name} |"; then
        add_error "Command Ledger missing required column: ${col_name}"
      fi
    done
  else
    add_error "Command Ledger table not found"
  fi

  # 12. Compatibility Inventory: header has required columns
  local compat_header
  compat_header=$(extract_table_header "$AUDIT_FILE" "Compatibility Inventory")
  if [[ -n "$compat_header" ]]; then
    for col_name in "${COMPAT_REQUIRED_COLS[@]}"; do
      if ! echo "$compat_header" | grep -qi "| ${col_name} |"; then
        add_error "Compatibility Inventory missing required column: ${col_name}"
      fi
    done
  else
    add_error "Compatibility Inventory table not found"
  fi

  # 13. Requirement Traceability: header has required columns
  local trace_header
  trace_header=$(extract_table_header "$AUDIT_FILE" "Requirement Traceability")
  if [[ -n "$trace_header" ]]; then
    for col_name in "${TRACEABILITY_REQUIRED_COLS[@]}"; do
      if ! echo "$trace_header" | grep -qi "| ${col_name} |"; then
        add_error "Requirement Traceability missing required column: ${col_name}"
      fi
    done
  else
    add_error "Requirement Traceability table not found"
  fi

  # 14. Live-source count/checksum assertion
  if [[ ! -f "$SOURCE_FILE" ]]; then
    add_error "Source file not found: $SOURCE_FILE"
  else
    # Count Self:: occurrences (should be >= 30: 15 into_internal + 15 from_internal)
    local self_count
    self_count=$(grep -c 'Self::' "$SOURCE_FILE" 2>/dev/null || echo 0)
    if [[ "$self_count" -lt 30 ]]; then
      add_error "Source Self:: count ($self_count) below expected minimum 30 (15 into_internal + 15 from_internal)"
    fi

    # Verify each variant name appears in the source file
    local variant
    for variant in "${VARIANTS[@]}"; do
      if ! grep -q "$variant" "$SOURCE_FILE" 2>/dev/null; then
        add_error "Variant '$variant' not found in source file: $SOURCE_FILE"
      fi
    done
  fi
}

# ---------------------------------------------------------------------------
# Evidence mode validations (in addition to schema)
# ---------------------------------------------------------------------------

validate_evidence() {
  # 1. Operation Matrix: no placeholder values in assessment columns
  local op_data_rows
  op_data_rows=$(extract_table_data_rows "$AUDIT_FILE" "Operation Matrix")
  local row
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local variant_name
    variant_name=$(get_cell "$row" 2)

    # Check assessment columns are non-placeholder
    # prod_callers=8, test_callers=9, impl=10, verify=11, disp=12, conf=13, evidence=14
    local prod_callers test_callers impl_val verify_val disp_val conf_val evidence_val
    prod_callers=$(get_cell "$row" 9)
    test_callers=$(get_cell "$row" 10)
    impl_val=$(get_cell "$row" 11)
    verify_val=$(get_cell "$row" 12)
    disp_val=$(get_cell "$row" 13)
    conf_val=$(get_cell "$row" 14)
    evidence_val=$(get_cell "$row" 15)

    if is_placeholder "$prod_callers"; then
      add_error "Operation Matrix: prod_callers is placeholder for variant: $variant_name"
    fi
    if is_placeholder "$test_callers"; then
      add_error "Operation Matrix: test_callers is placeholder for variant: $variant_name"
    fi
    if is_placeholder "$impl_val"; then
      add_error "Operation Matrix: impl is placeholder for variant: $variant_name"
    fi
    if is_placeholder "$verify_val"; then
      add_error "Operation Matrix: verify is placeholder for variant: $variant_name"
    fi
    if is_placeholder "$disp_val"; then
      add_error "Operation Matrix: disp is placeholder for variant: $variant_name"
    fi
    if is_placeholder "$conf_val"; then
      add_error "Operation Matrix: conf is placeholder for variant: $variant_name"
    fi
    if is_placeholder "$evidence_val"; then
      add_error "Operation Matrix: evidence is placeholder for variant: $variant_name"
    fi

    # Validate taxonomy values are from allowed set (not just non-empty)
    if ! echo "$IMPL_VALUES" | grep -qE "^${impl_val}$"; then
      add_error "Operation Matrix: impl '$impl_val' not in allowed taxonomy for variant: $variant_name"
    fi
    if ! echo "$VERIFY_VALUES" | grep -qE "^${verify_val}$"; then
      add_error "Operation Matrix: verify '$verify_val' not in allowed taxonomy for variant: $variant_name"
    fi
    if ! echo "$DISP_VALUES" | grep -qE "^${disp_val}$"; then
      add_error "Operation Matrix: disp '$disp_val' not in allowed taxonomy for variant: $variant_name"
    fi
    if ! echo "$CONF_VALUES" | grep -qE "^${conf_val}$"; then
      add_error "Operation Matrix: conf '$conf_val' not in allowed taxonomy for variant: $variant_name"
    fi
  done <<< "$op_data_rows"

  # 2. Every referenced evidence ID in Operation Matrix must exist in Evidence Index
  local registered_evidence
  registered_evidence=$(extract_table_data_rows "$AUDIT_FILE" "Evidence Index" | while IFS= read -r erow; do
    [[ -z "$erow" ]] && continue
    local eid
    eid=$(get_cell "$erow" 2)
    if ! is_placeholder "$eid"; then
      echo "$eid"
    fi
  done)

  # Check evidence IDs in Operation Matrix
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local evidence_val
    evidence_val=$(get_cell "$row" 15)
    if [[ -n "$evidence_val" ]] && [[ "$evidence_val" != "none" ]]; then
      IFS=',' read -ra ev_arr <<< "$evidence_val"
      for ev_id in "${ev_arr[@]}"; do
        ev_id=$(echo "$ev_id" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        if [[ -n "$ev_id" ]] && [[ -n "$registered_evidence" ]]; then
          if ! echo "$registered_evidence" | grep -qF "$ev_id"; then
            add_error "Evidence ID '$ev_id' referenced in Operation Matrix but not registered in Evidence Index"
          fi
        fi
      done
    fi
  done <<< "$op_data_rows"

  # 3. Command Ledger: all fields must be non-placeholder for real data rows
  local ledger_data
  ledger_data=$(extract_table_data_rows "$AUDIT_FILE" "Command Ledger")
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local first_cell
    first_cell=$(get_cell "$row" 2)
    if is_placeholder "$first_cell"; then
      add_error "Command Ledger contains placeholder row (evidence collection incomplete)"
      continue
    fi

    # All fields must be non-empty
    local cmd_val date_val exit_val test_count_val result_val
    cmd_val=$(get_cell "$row" 3)
    date_val=$(get_cell "$row" 4)
    exit_val=$(get_cell "$row" 5)
    test_count_val=$(get_cell "$row" 6)
    result_val=$(get_cell "$row" 7)

    [[ -z "$cmd_val" ]] && add_error "Command Ledger row '$first_cell': command is empty"
    [[ -z "$date_val" ]] && add_error "Command Ledger row '$first_cell': date is empty"
    [[ -z "$exit_val" ]] && add_error "Command Ledger row '$first_cell': exit status is empty"
    [[ -z "$test_count_val" ]] && add_error "Command Ledger row '$first_cell': test count is empty"
    [[ -z "$result_val" ]] && add_error "Command Ledger row '$first_cell': result is empty"

    # 4. Cargo ledger rows: test count must be a positive integer
    if [[ "$cmd_val" == *cargo* ]]; then
      if ! [[ "$test_count_val" =~ ^[0-9]+$ ]] || [[ "$test_count_val" -le 0 ]]; then
        add_error "Command Ledger Cargo row '$first_cell': test count '$test_count_val' is not a positive integer"
      fi
    fi
  done <<< "$ledger_data"

  # 5. Findings table: no placeholder rows
  local findings_data
  findings_data=$(extract_table_data_rows "$AUDIT_FILE" "Findings")
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local first_cell
    first_cell=$(get_cell "$row" 2)
    if is_placeholder "$first_cell"; then
      add_error "Findings table contains placeholder row (findings not yet collected)"
    fi
  done <<< "$findings_data"
}

# ---------------------------------------------------------------------------
# Final mode validations (in addition to evidence)
# ---------------------------------------------------------------------------

validate_final() {
  # 1. Audit Status must be final
  local status_line
  status_line=$(grep -i '^Audit Status:' "$AUDIT_FILE" 2>/dev/null | head -1)
  if ! echo "$status_line" | grep -qi 'final'; then
    add_error "Audit Status is not 'final' (found: $status_line)"
  fi

  # 2. Complete requirement traceability for AUDIT-01 through AUDIT-03
  local trace_data
  trace_data=$(extract_table_data_rows "$AUDIT_FILE" "Requirement Traceability")
  for req_id in "AUDIT-01" "AUDIT-02" "AUDIT-03"; do
    local req_row
    req_row=$(echo "$trace_data" | grep -F "$req_id" || true)
    if [[ -z "$req_row" ]]; then
      add_error "Requirement Traceability missing: $req_id"
    else
      local req_status
      req_status=$(get_cell "$req_row" 4)
      if [[ "$req_status" != "complete" ]] && [[ "$req_status" != "pass" ]]; then
        add_error "Requirement Traceability: $req_id status is '$req_status', expected 'complete'"
      fi
    fi
  done

  # 3. All four finding categories represented (where live evidence supports them)
  local findings_data
  findings_data=$(extract_table_data_rows "$AUDIT_FILE" "Findings")
  local found_blocking=0 found_required=0 found_hardening=0 found_informational=0
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local first_cell oblig_val
    first_cell=$(get_cell "$row" 2)
    if is_placeholder "$first_cell"; then
      continue
    fi
    oblig_val=$(get_cell "$row" 3)
    case "$oblig_val" in
      blocking)      found_blocking=1 ;;
      required)      found_required=1 ;;
      hardening)     found_hardening=1 ;;
      informational) found_informational=1 ;;
    esac
  done <<< "$findings_data"

  [[ $found_blocking -eq 0 ]]      && add_error "Findings: no 'blocking' finding represented"
  [[ $found_required -eq 0 ]]      && add_error "Findings: no 'required' finding represented"
  [[ $found_hardening -eq 0 ]]     && add_error "Findings: no 'hardening' finding represented"
  [[ $found_informational -eq 0 ]] && add_error "Findings: no 'informational' finding represented"

  # 4. Zero blockers across all Operation Matrix rows and Findings
  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local variant_name blockers_val
    variant_name=$(get_cell "$row" 2)
    blockers_val=$(get_cell "$row" 17)
    if [[ "$blockers_val" != "none" ]] && [[ -n "$blockers_val" ]]; then
      add_error "Final mode: variant '$variant_name' has blockers: $blockers_val"
    fi
  done <<< "$(extract_table_data_rows "$AUDIT_FILE" "Operation Matrix")"

  while IFS= read -r row; do
    [[ -z "$row" ]] && continue
    local first_cell blockers_val
    first_cell=$(get_cell "$row" 2)
    if is_placeholder "$first_cell"; then
      continue
    fi
    blockers_val=$(get_cell "$row" 12)  # blockers is field 12 in findings (11 columns + 1)
    if [[ "$blockers_val" != "none" ]] && [[ -n "$blockers_val" ]]; then
      add_error "Final mode: finding '$first_cell' has blockers: $blockers_val"
    fi
  done <<< "$(extract_table_data_rows "$AUDIT_FILE" "Findings")"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
  echo "=== validate-audit.sh ==="
  echo "Mode: $MODE"
  echo "Audit file: $AUDIT_FILE"
  echo "Source file: $SOURCE_FILE"
  echo ""

  # Schema mode always runs
  validate_schema

  # Evidence mode adds evidence completeness checks
  if [[ "$MODE" == "evidence" ]] || [[ "$MODE" == "final" ]]; then
    validate_evidence
  fi

  # Final mode adds closure checks
  if [[ "$MODE" == "final" ]]; then
    validate_final
  fi

  # Report warnings
  if [[ ${#WARNINGS[@]} -gt 0 ]]; then
    echo "--- Warnings ---"
    for w in "${WARNINGS[@]}"; do
      echo "  $w"
    done
    echo ""
  fi

  # Report errors
  if [[ ${#ERRORS[@]} -gt 0 ]]; then
    echo "--- Errors (${#ERRORS[@]}) ---"
    for e in "${ERRORS[@]}"; do
      echo "  $e"
    done
    echo ""
    echo "RESULT: FAIL ($MODE mode, ${#ERRORS[@]} errors)"
    exit 1
  fi

  echo "RESULT: PASS ($MODE mode)"
  exit 0
}

main
