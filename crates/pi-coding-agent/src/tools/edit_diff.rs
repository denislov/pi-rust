#[derive(Debug, Clone, PartialEq, Eq)]
enum DiffPart {
    Equal(Vec<String>),
    Removed(Vec<String>),
    Added(Vec<String>),
}

pub(crate) struct DiffString {
    pub diff: String,
    pub first_changed_line: Option<usize>,
}

fn split_lines(content: &str) -> Vec<String> {
    let mut lines = content.split('\n').map(str::to_string).collect::<Vec<_>>();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

fn diff_parts(old_content: &str, new_content: &str) -> Vec<DiffPart> {
    let old_lines = split_lines(old_content);
    let new_lines = split_lines(new_content);
    let old_len = old_lines.len();
    let new_len = new_lines.len();
    let mut lcs = vec![vec![0usize; new_len + 1]; old_len + 1];

    for old_idx in (0..old_len).rev() {
        for new_idx in (0..new_len).rev() {
            lcs[old_idx][new_idx] = if old_lines[old_idx] == new_lines[new_idx] {
                lcs[old_idx + 1][new_idx + 1] + 1
            } else {
                lcs[old_idx + 1][new_idx].max(lcs[old_idx][new_idx + 1])
            };
        }
    }

    let mut raw_parts = Vec::new();
    let (mut old_idx, mut new_idx) = (0usize, 0usize);
    while old_idx < old_len && new_idx < new_len {
        if old_lines[old_idx] == new_lines[new_idx] {
            raw_parts.push(DiffPart::Equal(vec![old_lines[old_idx].clone()]));
            old_idx += 1;
            new_idx += 1;
        } else if lcs[old_idx + 1][new_idx] >= lcs[old_idx][new_idx + 1] {
            raw_parts.push(DiffPart::Removed(vec![old_lines[old_idx].clone()]));
            old_idx += 1;
        } else {
            raw_parts.push(DiffPart::Added(vec![new_lines[new_idx].clone()]));
            new_idx += 1;
        }
    }
    while old_idx < old_len {
        raw_parts.push(DiffPart::Removed(vec![old_lines[old_idx].clone()]));
        old_idx += 1;
    }
    while new_idx < new_len {
        raw_parts.push(DiffPart::Added(vec![new_lines[new_idx].clone()]));
        new_idx += 1;
    }

    coalesce_parts(raw_parts)
}

fn coalesce_parts(parts: Vec<DiffPart>) -> Vec<DiffPart> {
    let mut coalesced: Vec<DiffPart> = Vec::new();
    for part in parts {
        match (coalesced.last_mut(), part) {
            (Some(DiffPart::Equal(existing)), DiffPart::Equal(mut lines))
            | (Some(DiffPart::Removed(existing)), DiffPart::Removed(mut lines))
            | (Some(DiffPart::Added(existing)), DiffPart::Added(mut lines)) => {
                existing.append(&mut lines);
            }
            (_, part) => coalesced.push(part),
        }
    }
    coalesced
}

pub(crate) fn generate_diff_string(
    old_content: &str,
    new_content: &str,
    context_lines: usize,
) -> DiffString {
    let parts = diff_parts(old_content, new_content);
    let old_lines = split_lines(old_content);
    let new_lines = split_lines(new_content);
    let max_line_num = old_lines.len().max(new_lines.len()).max(1);
    let line_num_width = max_line_num.to_string().len();

    let mut output = Vec::new();
    let mut old_line_num = 1usize;
    let mut new_line_num = 1usize;
    let mut last_was_change = false;
    let mut first_changed_line = None;

    for (index, part) in parts.iter().enumerate() {
        match part {
            DiffPart::Added(lines) | DiffPart::Removed(lines) => {
                if first_changed_line.is_none() {
                    first_changed_line = Some(new_line_num);
                }

                for line in lines {
                    match part {
                        DiffPart::Added(_) => {
                            output.push(format!(
                                "+{:>width$} {}",
                                new_line_num,
                                line,
                                width = line_num_width
                            ));
                            new_line_num += 1;
                        }
                        DiffPart::Removed(_) => {
                            output.push(format!(
                                "-{:>width$} {}",
                                old_line_num,
                                line,
                                width = line_num_width
                            ));
                            old_line_num += 1;
                        }
                        DiffPart::Equal(_) => unreachable!(),
                    }
                }
                last_was_change = true;
            }
            DiffPart::Equal(lines) => {
                let next_part_is_change = parts
                    .get(index + 1)
                    .is_some_and(|part| !matches!(part, DiffPart::Equal(_)));
                let has_leading_change = last_was_change;
                let has_trailing_change = next_part_is_change;

                if has_leading_change && has_trailing_change {
                    if lines.len() <= context_lines * 2 {
                        push_context_lines(
                            &mut output,
                            lines,
                            &mut old_line_num,
                            &mut new_line_num,
                            line_num_width,
                        );
                    } else {
                        let leading = &lines[..context_lines];
                        let trailing = &lines[lines.len() - context_lines..];
                        let skipped = lines.len() - leading.len() - trailing.len();
                        push_context_lines(
                            &mut output,
                            leading,
                            &mut old_line_num,
                            &mut new_line_num,
                            line_num_width,
                        );
                        output.push(format!(" {:>width$} ...", "", width = line_num_width));
                        old_line_num += skipped;
                        new_line_num += skipped;
                        push_context_lines(
                            &mut output,
                            trailing,
                            &mut old_line_num,
                            &mut new_line_num,
                            line_num_width,
                        );
                    }
                } else if has_leading_change {
                    let shown = lines.len().min(context_lines);
                    push_context_lines(
                        &mut output,
                        &lines[..shown],
                        &mut old_line_num,
                        &mut new_line_num,
                        line_num_width,
                    );
                    let skipped = lines.len() - shown;
                    if skipped > 0 {
                        output.push(format!(" {:>width$} ...", "", width = line_num_width));
                        old_line_num += skipped;
                        new_line_num += skipped;
                    }
                } else if has_trailing_change {
                    let skipped = lines.len().saturating_sub(context_lines);
                    if skipped > 0 {
                        output.push(format!(" {:>width$} ...", "", width = line_num_width));
                        old_line_num += skipped;
                        new_line_num += skipped;
                    }
                    push_context_lines(
                        &mut output,
                        &lines[skipped..],
                        &mut old_line_num,
                        &mut new_line_num,
                        line_num_width,
                    );
                } else {
                    old_line_num += lines.len();
                    new_line_num += lines.len();
                }

                last_was_change = false;
            }
        }
    }

    DiffString {
        diff: output.join("\n"),
        first_changed_line,
    }
}

fn push_context_lines(
    output: &mut Vec<String>,
    lines: &[String],
    old_line_num: &mut usize,
    new_line_num: &mut usize,
    line_num_width: usize,
) {
    for line in lines {
        output.push(format!(
            " {:>width$} {}",
            *old_line_num,
            line,
            width = line_num_width
        ));
        *old_line_num += 1;
        *new_line_num += 1;
    }
}

pub(crate) fn generate_unified_patch(path: &str, old_content: &str, new_content: &str) -> String {
    let old_lines = split_lines(old_content);
    let new_lines = split_lines(new_content);
    let old_count = old_lines.len();
    let new_count = new_lines.len();
    let mut output = vec![
        format!("--- {path}"),
        format!("+++ {path}"),
        format!("@@ -1,{old_count} +1,{new_count} @@"),
    ];

    for part in diff_parts(old_content, new_content) {
        match part {
            DiffPart::Equal(lines) => {
                output.extend(lines.into_iter().map(|line| format!(" {line}")));
            }
            DiffPart::Removed(lines) => {
                output.extend(lines.into_iter().map(|line| format!("-{line}")));
            }
            DiffPart::Added(lines) => {
                output.extend(lines.into_iter().map(|line| format!("+{line}")));
            }
        }
    }

    output.join("\n")
}
