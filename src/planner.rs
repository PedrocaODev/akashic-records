use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::graph::normalize_posix_path;
use crate::model::{Plan, PlanAction};

// --- Pure Rust SHA-256 (FIPS 180-4) ---

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Computes the standard lowercase hexadecimal SHA-256 digest of input bytes.
pub fn sha256_digest(bytes: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    let mut data = bytes.to_vec();
    data.push(0x80);
    while (data.len() % 64) != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_var = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_var
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_var = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_var);
    }

    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

// --- Unified Diff Generation & Application ---

/// Generates a standard unified diff string between original and modified file contents.
pub fn generate_unified_diff(path: &str, original: &str, modified: &str) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();
    let from_file = format!("a/{}", path);
    let to_file = format!("b/{}", path);
    let diff = difflib::unified_diff(&orig_lines, &mod_lines, &from_file, &to_file, "", "", 3);
    let mut out = diff.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Applies a unified diff to the original text.
pub fn apply_diff(original: &str, diff: &str) -> Result<String, String> {
    if diff.trim().is_empty() {
        return Ok(original.to_string());
    }

    let orig_lines: Vec<&str> = original.lines().collect();
    let diff_lines: Vec<&str> = diff.lines().collect();

    let mut result_lines: Vec<String> = Vec::new();
    let mut orig_idx = 0; // 0-based index into orig_lines

    let mut i = 0;
    while i < diff_lines.len() {
        let line = diff_lines[i];
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            i += 1;
            continue;
        }

        if line.starts_with("@@ ") {
            // Hunk header: @@ -old_start,old_count +new_start,new_count @@
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return Err(format!("Malformed hunk header: {}", line));
            }
            let old_spec = parts[1]
                .strip_prefix('-')
                .ok_or_else(|| format!("Invalid hunk header: {}", line))?;
            let old_start: usize = match old_spec.split_once(',') {
                Some((start_str, _)) => start_str
                    .parse()
                    .map_err(|e| format!("Invalid start line: {}", e))?,
                None => old_spec
                    .parse()
                    .map_err(|e| format!("Invalid start line: {}", e))?,
            };

            let target_orig_idx = if old_start == 0 { 0 } else { old_start - 1 };

            while orig_idx < target_orig_idx && orig_idx < orig_lines.len() {
                result_lines.push(orig_lines[orig_idx].to_string());
                orig_idx += 1;
            }

            i += 1;
            while i < diff_lines.len() && !diff_lines[i].starts_with("@@ ") {
                let hline = diff_lines[i];
                if let Some(content) = hline.strip_prefix(' ') {
                    result_lines.push(content.to_string());
                    if orig_idx < orig_lines.len() {
                        orig_idx += 1;
                    }
                } else if let Some(_content) = hline.strip_prefix('-') {
                    if orig_idx < orig_lines.len() {
                        orig_idx += 1;
                    }
                } else if let Some(content) = hline.strip_prefix('+') {
                    result_lines.push(content.to_string());
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    while orig_idx < orig_lines.len() {
        result_lines.push(orig_lines[orig_idx].to_string());
        orig_idx += 1;
    }

    let mut out = result_lines.join("\n");
    if original.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }

    Ok(out)
}

// --- Path & Link Resolution ---

/// Converts an arbitrary string into a kebab-case slug suitable for a node ID.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if c == '-' || c == '_' || c.is_whitespace() {
            if !prev_dash && !out.is_empty() {
                out.push('-');
                prev_dash = true;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

/// Computes the relative POSIX path from a source file's directory to a target file.
pub fn make_relative_link(source_file_rel: &str, target_file_rel: &str) -> String {
    let source_dir = Path::new(source_file_rel)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let source_dir = if source_dir == "." { "" } else { &source_dir };

    let target_norm = target_file_rel.replace('\\', "/");
    let target_path = Path::new(&target_norm);
    let target_dir = target_path
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let target_dir = if target_dir == "." { "" } else { &target_dir };
    let filename = target_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| target_norm.clone());

    if source_dir == target_dir {
        return format!("./{}", filename);
    }

    let source_parts: Vec<&str> = if source_dir.is_empty() {
        Vec::new()
    } else {
        source_dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    let target_parts: Vec<&str> = if target_dir.is_empty() {
        Vec::new()
    } else {
        target_dir.split('/').filter(|s| !s.is_empty()).collect()
    };

    let mut common = 0;
    while common < source_parts.len()
        && common < target_parts.len()
        && source_parts[common] == target_parts[common]
    {
        common += 1;
    }

    let mut comps = Vec::new();
    for _ in common..source_parts.len() {
        comps.push("..");
    }
    for comp in &target_parts[common..] {
        comps.push(comp);
    }
    comps.push(&filename);

    let res = comps.join("/");
    if !res.starts_with('.') {
        format!("./{}", res)
    } else {
        res
    }
}

/// Resolves a target reference (from a wikilink or supersedes target) relative to a source note.
pub fn resolve_wikilink_target(
    source_file_rel: &str,
    target: &str,
    vault_paths: &[String],
) -> String {
    let target_clean = target.trim();
    if target_clean.is_empty() {
        return target_clean.to_string();
    }

    // Split target into base and anchor
    let (base, anchor) = match target_clean.split_once('#') {
        Some((b, a)) => (b.trim(), Some(a.trim())),
        None => (target_clean, None),
    };

    if base.is_empty() {
        return anchor.map(|a| format!("#{}", a)).unwrap_or_default();
    }

    let clean_base = base.replace('\\', "/");
    let with_md = format!("{}.md", clean_base);

    // Check if clean_base matches any file in vault
    let mut matched_path = None;
    for vp in vault_paths {
        if vp == &clean_base || vp.eq_ignore_ascii_case(&clean_base) {
            matched_path = Some(vp.clone());
            break;
        }
        if vp == &with_md || vp.eq_ignore_ascii_case(&with_md) {
            matched_path = Some(vp.clone());
            break;
        }
        // Match by filename stem
        if let Some(stem) = Path::new(vp).file_stem().and_then(|s| s.to_str()) {
            if stem == clean_base || stem.eq_ignore_ascii_case(&clean_base) {
                matched_path = Some(vp.clone());
                break;
            }
        }
        // Match by filename
        if let Some(fname) = Path::new(vp).file_name().and_then(|s| s.to_str()) {
            if fname == clean_base || fname.eq_ignore_ascii_case(&clean_base) {
                matched_path = Some(vp.clone());
                break;
            }
        }
    }

    let resolved_base = match matched_path {
        Some(ref dest) => make_relative_link(source_file_rel, dest),
        None => {
            let base_with_ext = if clean_base.ends_with(".md") || clean_base.ends_with(".markdown")
            {
                clean_base
            } else {
                format!("{}.md", clean_base)
            };
            if base_with_ext.starts_with("./") || base_with_ext.starts_with("../") {
                base_with_ext
            } else {
                format!("./{}", base_with_ext)
            }
        }
    };

    match anchor {
        Some(a) if !a.is_empty() => format!("{}#{}", resolved_base, a),
        _ => resolved_base,
    }
}

// --- Content Normalization ---

/// Replaces legacy Obsidian wikilinks `[[...]]` with standard Markdown links outside code blocks.
pub fn normalize_wikilinks_in_body(
    body: &str,
    source_path: &str,
    vault_paths: &[String],
) -> (String, bool) {
    let mut modified = false;
    let mut out_lines = Vec::new();
    let mut inside_code_fence = false;

    for line in body.lines() {
        let trimmed_line = line.trim_start();
        if trimmed_line.starts_with("```") || trimmed_line.starts_with("~~~") {
            inside_code_fence = !inside_code_fence;
            out_lines.push(line.to_string());
            continue;
        }

        if inside_code_fence || !line.contains("[[") {
            out_lines.push(line.to_string());
            continue;
        }

        // Process wikilinks on this line
        let mut new_line = String::new();
        let mut cursor = 0;
        let line_chars: Vec<char> = line.chars().collect();
        let len = line_chars.len();

        while cursor < len {
            if cursor + 1 < len && line_chars[cursor] == '[' && line_chars[cursor + 1] == '[' {
                if let Some(end_rel) = line_chars[cursor + 2..]
                    .windows(2)
                    .position(|w| w[0] == ']' && w[1] == ']')
                {
                    let inner: String = line_chars[cursor + 2..cursor + 2 + end_rel]
                        .iter()
                        .collect();
                    let inner_trimmed = inner.trim();

                    let (target_part, text_part) = match inner_trimmed.split_once('|') {
                        Some((t, txt)) => (t.trim(), Some(txt.trim())),
                        None => (inner_trimmed, None),
                    };

                    let resolved = resolve_wikilink_target(source_path, target_part, vault_paths);
                    let display_text = text_part.unwrap_or(target_part);

                    new_line.push_str(&format!("[{}]({})", display_text, resolved));
                    modified = true;
                    cursor += 2 + end_rel + 2;
                    continue;
                }
            }
            new_line.push(line_chars[cursor]);
            cursor += 1;
        }

        out_lines.push(new_line);
    }

    let mut result = out_lines.join("\n");
    if body.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }

    (result, modified)
}

/// Normalizes a single note's content for missing IDs, top-level `supersedes:`, and wikilinks.
/// Returns `Some(normalized_content)` if changes were made, or `None` if the note is already compliant.
pub fn normalize_note_content(
    content: &str,
    source_path: &str,
    vault_paths: &[String],
) -> Result<Option<String>, String> {
    let trimmed = content.trim_start_matches('\u{feff}');
    let file_stem = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");

    // Case 1: Missing frontmatter delimiter entirely
    if !trimmed.starts_with("---") {
        let (norm_body, _) = normalize_wikilinks_in_body(trimmed, source_path, vault_paths);
        let id = slugify(file_stem);

        // Find title from first heading or fallback to file stem
        let title = norm_body
            .lines()
            .find(|l| l.trim_start().starts_with("# "))
            .map(|l| l.trim_start()[2..].trim().to_string())
            .unwrap_or_else(|| file_stem.to_string());

        let new_content = format!(
            "---\nid: {}\ntype: Concept\ntitle: {}\n---\n{}",
            id, title, norm_body
        );
        return Ok(Some(new_content));
    }

    // Parse frontmatter delimiters
    let rest = &trimmed[3..];
    let after_first_line = match rest.find('\n') {
        Some(pos) => &rest[pos + 1..],
        None => return Ok(None),
    };

    let mut closing_pos = None;
    let mut current_offset = 0;
    for line in after_first_line.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]).trim();
        if trimmed_line == "---" || trimmed_line == "..." {
            closing_pos = Some(current_offset);
            break;
        }
        current_offset += line.len();
    }

    let closing_offset = match closing_pos {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let yaml_str = &after_first_line[..closing_offset];
    let body_start = match after_first_line[closing_offset..].find('\n') {
        Some(pos) => closing_offset + pos + 1,
        None => after_first_line.len(),
    };
    let body_str = &after_first_line[body_start..];

    // Check if YAML parses
    let yaml_val: serde_yaml::Value =
        serde_yaml::from_str(yaml_str).map_err(|e| format!("YAML parsing error: {}", e))?;

    let mapping = match yaml_val {
        serde_yaml::Value::Mapping(m) => m,
        _ => return Ok(None),
    };

    let mut needs_changes = false;

    // Check 1: Missing or empty ID
    let has_valid_id = match mapping.get(&serde_yaml::Value::String("id".to_string())) {
        Some(serde_yaml::Value::String(s)) => !s.trim().is_empty(),
        _ => false,
    };
    if !has_valid_id {
        needs_changes = true;
    }

    // Check 2: Top-level supersedes field
    let has_top_level_supersedes =
        mapping.contains_key(&serde_yaml::Value::String("supersedes".to_string()));
    if has_top_level_supersedes {
        needs_changes = true;
    }

    // Check 3: Wikilinks in frontmatter
    let yaml_has_wikilinks = yaml_str.contains("[[") && yaml_str.contains("]]");
    if yaml_has_wikilinks {
        needs_changes = true;
    }

    // Check 4: Wikilinks in body
    let (norm_body, body_has_wikilinks) =
        normalize_wikilinks_in_body(body_str, source_path, vault_paths);
    if body_has_wikilinks {
        needs_changes = true;
    }

    if !needs_changes {
        return Ok(None);
    }

    // Perform transformations
    let mut new_mapping = serde_yaml::Mapping::new();

    // 1. Ensure ID is present
    let id_key = serde_yaml::Value::String("id".to_string());
    let id_val = if has_valid_id {
        mapping.get(&id_key).unwrap().clone()
    } else {
        serde_yaml::Value::String(slugify(file_stem))
    };
    new_mapping.insert(id_key, id_val);

    // 2. Extract top-level supersedes targets if present
    let mut extra_supersedes = Vec::new();
    if let Some(super_val) = mapping.get(&serde_yaml::Value::String("supersedes".to_string())) {
        match super_val {
            serde_yaml::Value::String(s) => {
                let target = resolve_wikilink_target(source_path, s, vault_paths);
                extra_supersedes.push(target);
            }
            serde_yaml::Value::Sequence(seq) => {
                for item in seq {
                    if let serde_yaml::Value::String(s) = item {
                        let target = resolve_wikilink_target(source_path, s, vault_paths);
                        extra_supersedes.push(target);
                    }
                }
            }
            _ => {}
        }
    }

    // 3. Copy other keys in canonical order, omitting top-level supersedes
    let standard_keys = [
        "type",
        "title",
        "status",
        "valid_from",
        "valid_until",
        "scope",
        "relations",
    ];

    for key_name in &standard_keys {
        let key = serde_yaml::Value::String(key_name.to_string());
        if *key_name == "relations" {
            // Merge existing relations with extra_supersedes
            let mut rel_seq = match mapping.get(&key) {
                Some(serde_yaml::Value::Sequence(seq)) => seq.clone(),
                _ => Vec::new(),
            };

            for target in extra_supersedes.drain(..) {
                // Check if already in relations
                let exists = rel_seq.iter().any(|rel_val| {
                    if let serde_yaml::Value::Mapping(rel_m) = rel_val {
                        let t_match = rel_m
                            .get(&serde_yaml::Value::String("type".to_string()))
                            .and_then(|v| v.as_str())
                            == Some("supersedes");
                        let tgt_match = rel_m
                            .get(&serde_yaml::Value::String("target".to_string()))
                            .and_then(|v| v.as_str())
                            == Some(&target);
                        t_match && tgt_match
                    } else {
                        false
                    }
                });

                if !exists {
                    let mut rel_item = serde_yaml::Mapping::new();
                    rel_item.insert(
                        serde_yaml::Value::String("type".to_string()),
                        serde_yaml::Value::String("supersedes".to_string()),
                    );
                    rel_item.insert(
                        serde_yaml::Value::String("target".to_string()),
                        serde_yaml::Value::String(target),
                    );
                    rel_seq.push(serde_yaml::Value::Mapping(rel_item));
                }
            }

            // Normalize wikilinks in relations if any
            for rel_val in &mut rel_seq {
                if let serde_yaml::Value::Mapping(rel_m) = rel_val {
                    let target_key = serde_yaml::Value::String("target".to_string());
                    if let Some(serde_yaml::Value::String(tgt)) = rel_m.get(&target_key) {
                        let resolved = resolve_wikilink_target(source_path, tgt, vault_paths);
                        rel_m.insert(target_key, serde_yaml::Value::String(resolved));
                    }
                }
            }

            if !rel_seq.is_empty() {
                new_mapping.insert(key, serde_yaml::Value::Sequence(rel_seq));
            }
        } else if let Some(val) = mapping.get(&key) {
            new_mapping.insert(key, val.clone());
        }
    }

    // Copy any remaining custom keys (excluding id, supersedes, and the standard keys)
    for (k, v) in mapping {
        if let serde_yaml::Value::String(ref s) = k {
            if s == "id" || s == "supersedes" || standard_keys.contains(&s.as_str()) {
                continue;
            }
        }
        new_mapping.insert(k, v);
    }

    let new_yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(new_mapping))
        .map_err(|e| format!("YAML serialization error: {}", e))?;

    let final_content = format!("---\n{}---\n{}", new_yaml, norm_body);

    if final_content == content {
        Ok(None)
    } else {
        Ok(Some(final_content))
    }
}

// --- Plan & Apply Core Engine ---

/// Result of an apply run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub files_modified: usize,
    pub files_already_up_to_date: usize,
}

/// Helper to determine whether a WalkDir entry is a hidden file or directory.
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() > 0 {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with('.') {
                return true;
            }
        }
    }
    false
}

/// Inspects a vault directory and generates a migration plan with unified diffs.
pub fn plan_vault<P: AsRef<Path>>(vault_dir: P) -> Result<Plan, Box<dyn std::error::Error>> {
    let dir_ref = vault_dir.as_ref();
    if !dir_ref.exists() {
        return Err(format!("Vault directory not found: {}", dir_ref.display()).into());
    }
    if !dir_ref.is_dir() {
        return Err(format!("Path is not a directory: {}", dir_ref.display()).into());
    }

    let canonical_root = dir_ref
        .canonicalize()
        .unwrap_or_else(|_| dir_ref.to_path_buf())
        .to_string_lossy()
        .to_string();

    // Step 1: Collect all markdown file paths in vault
    let mut md_files: Vec<(PathBuf, String)> = Vec::new();
    for entry_res in WalkDir::new(dir_ref)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        let entry = entry_res?;
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                    let rel_path = match path.strip_prefix(dir_ref) {
                        Ok(p) => p,
                        Err(_) => path,
                    };
                    let rel_posix = normalize_posix_path(&rel_path.to_string_lossy())
                        .unwrap_or_else(|| rel_path.to_string_lossy().replace('\\', "/"));
                    md_files.push((path.to_path_buf(), rel_posix));
                }
            }
        }
    }

    let vault_paths: Vec<String> = md_files.iter().map(|(_, p)| p.clone()).collect();
    let mut actions = Vec::new();

    // Step 2: Inspect each note and propose diffs if needed
    for (abs_path, rel_posix) in md_files {
        let original_content = fs::read_to_string(&abs_path)?;
        let source_sha = sha256_digest(original_content.as_bytes());

        match normalize_note_content(&original_content, &rel_posix, &vault_paths) {
            Ok(Some(new_content)) => {
                let expected_new_sha = sha256_digest(new_content.as_bytes());
                let diff = generate_unified_diff(&rel_posix, &original_content, &new_content);

                actions.push(PlanAction {
                    path: rel_posix,
                    source_sha256: source_sha,
                    expected_new_sha256: expected_new_sha,
                    diff,
                });
            }
            Ok(None) => {
                // Note is already compliant; no action needed
            }
            Err(err) => {
                eprintln!(
                    "Warning: Could not plan normalization for '{}': {}",
                    rel_posix, err
                );
            }
        }
    }

    // Sort actions deterministically by relative path
    actions.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Plan {
        plan_version: "0.1.0".to_string(),
        vault_root: canonical_root,
        actions,
    })
}

/// Applies an approved migration plan to a vault idempotently.
pub fn apply_plan<P: AsRef<Path>>(
    vault_dir: P,
    plan: &Plan,
    dry_run: bool,
) -> Result<ApplyResult, Box<dyn std::error::Error>> {
    let dir_ref = vault_dir.as_ref();
    if !dir_ref.exists() {
        return Err(format!("Vault directory not found: {}", dir_ref.display()).into());
    }

    // Phase 1: Pre-validate all target files before modifying anything
    for action in &plan.actions {
        let target_path = dir_ref.join(&action.path);
        if !target_path.exists() {
            return Err(format!("Target file not found: {}", target_path.display()).into());
        }

        let content = fs::read_to_string(&target_path)?;
        let current_sha = sha256_digest(content.as_bytes());

        if current_sha != action.source_sha256 && current_sha != action.expected_new_sha256 {
            return Err(format!(
                "Hash mismatch for file '{}': expected source SHA-256 '{}', found '{}'. Vault was modified after plan generation. Aborting cleanly without modifications.",
                action.path, action.source_sha256, current_sha
            )
            .into());
        }
    }

    // Phase 2: If dry-run, calculate stats and exit without modifying files
    if dry_run {
        let mut files_modified = 0;
        let mut files_already_up_to_date = 0;

        for action in &plan.actions {
            let target_path = dir_ref.join(&action.path);
            let content = fs::read_to_string(&target_path)?;
            let current_sha = sha256_digest(content.as_bytes());

            if current_sha == action.source_sha256 {
                files_modified += 1;
            } else if current_sha == action.expected_new_sha256 {
                files_already_up_to_date += 1;
            }
        }

        return Ok(ApplyResult {
            files_modified,
            files_already_up_to_date,
        });
    }

    // Phase 3: Apply modifications
    let mut files_modified = 0;
    let mut files_already_up_to_date = 0;

    for action in &plan.actions {
        let target_path = dir_ref.join(&action.path);
        let current_content = fs::read_to_string(&target_path)?;
        let current_sha = sha256_digest(current_content.as_bytes());

        if current_sha == action.source_sha256 {
            let patched_content = apply_diff(&current_content, &action.diff)?;
            let new_sha = sha256_digest(patched_content.as_bytes());

            if new_sha != action.expected_new_sha256 {
                return Err(format!(
                    "Post-diff hash mismatch for file '{}': expected '{}', got '{}'",
                    action.path, action.expected_new_sha256, new_sha
                )
                .into());
            }

            fs::write(&target_path, patched_content)?;
            files_modified += 1;
        } else if current_sha == action.expected_new_sha256 {
            // Already up-to-date; zero file churn
            files_already_up_to_date += 1;
        }
    }

    Ok(ApplyResult {
        files_modified,
        files_already_up_to_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sha256_known_vectors() {
        assert_eq!(
            sha256_digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_digest(b"The quick brown fox jumps over the lazy dog"),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("My Note"), "my-note");
        assert_eq!(slugify("decision_123_test"), "decision-123-test");
        assert_eq!(slugify("already-slug"), "already-slug");
        assert_eq!(slugify("   "), "untitled");
    }

    #[test]
    fn test_make_relative_link() {
        assert_eq!(
            make_relative_link("decisions/adr-1.md", "decisions/adr-2.md"),
            "./adr-2.md"
        );
        assert_eq!(
            make_relative_link("decisions/adr-1.md", "reviews/rev-1.md"),
            "../reviews/rev-1.md"
        );
        assert_eq!(
            make_relative_link("root.md", "reviews/rev-1.md"),
            "./reviews/rev-1.md"
        );
        assert_eq!(make_relative_link("a/b/c.md", "root.md"), "../../root.md");
    }

    #[test]
    fn test_diff_and_apply_diff_roundtrip() {
        let orig = "line 1\nline 2\nline 3\nline 4\n";
        let modified = "line 1\nline 2 modified\nline 3\nline 4\nline 5\n";

        let diff = generate_unified_diff("test.txt", orig, modified);
        assert!(diff.contains("@@"));

        let patched = apply_diff(orig, &diff).unwrap();
        assert_eq!(patched, modified);
        assert_eq!(
            sha256_digest(patched.as_bytes()),
            sha256_digest(modified.as_bytes())
        );
    }

    #[test]
    fn test_normalize_missing_id() {
        let content = r#"---
type: Decision
title: Missing ID Note
---
Body text.
"#;
        let res = normalize_note_content(content, "decisions/my-note.md", &[]).unwrap();
        assert!(res.is_some());
        let normalized = res.unwrap();
        assert!(normalized.contains("id: my-note"));
        assert!(normalized.contains("title: Missing ID Note"));
    }

    #[test]
    fn test_normalize_top_level_supersedes() {
        let content = r#"---
id: adr-002
type: Decision
title: Use Events
supersedes: ./adr-001.md
---
Body
"#;
        let res = normalize_note_content(content, "decisions/adr-002.md", &[]).unwrap();
        assert!(res.is_some());
        let normalized = res.unwrap();
        assert!(!normalized.contains("supersedes: ./adr-001.md"));
        assert!(normalized.contains("relations:"));
        assert!(normalized.contains("- type: supersedes"));
        assert!(normalized.contains("target: ./adr-001.md"));
    }

    #[test]
    fn test_normalize_wikilinks_in_body() {
        let content = r#"---
id: adr-001
type: Decision
title: Use Rust
---
# Content
See [[adr-002]] and [[adr-002|Second Decision]] or [[#heading|Internal]].
```markdown
Do not replace [[inside_code]]
```
"#;
        let vault_paths = vec!["decisions/adr-002.md".to_string()];
        let res = normalize_note_content(content, "decisions/adr-001.md", &vault_paths).unwrap();
        assert!(res.is_some());
        let normalized = res.unwrap();
        assert!(normalized.contains("[adr-002](./adr-002.md)"));
        assert!(normalized.contains("[Second Decision](./adr-002.md)"));
        assert!(normalized.contains("[Internal](#heading)"));
        assert!(normalized.contains("[[inside_code]]"));
    }

    #[test]
    fn test_plan_and_apply_lifecycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create a note with missing ID, top-level supersedes, and wikilink
        let note = r#"---
type: Decision
title: Legacy Note
supersedes: ./old-note.md
---
Check [[old-note]].
"#;
        let note_path = root.join("legacy.md");
        fs::write(&note_path, note).unwrap();

        // Generate plan
        let plan = plan_vault(root).unwrap();
        assert_eq!(plan.actions.len(), 1);
        let action = &plan.actions[0];
        assert_eq!(action.path, "legacy.md");
        assert_eq!(action.source_sha256, sha256_digest(note.as_bytes()));
        assert!(!action.diff.is_empty());

        // Dry run: files should not change
        let dry_res = apply_plan(root, &plan, true).unwrap();
        assert_eq!(dry_res.files_modified, 1);
        assert_eq!(dry_res.files_already_up_to_date, 0);
        let content_after_dry = fs::read_to_string(&note_path).unwrap();
        assert_eq!(content_after_dry, note);

        // Apply plan
        let apply_res = apply_plan(root, &plan, false).unwrap();
        assert_eq!(apply_res.files_modified, 1);
        assert_eq!(apply_res.files_already_up_to_date, 0);

        let modified_content = fs::read_to_string(&note_path).unwrap();
        assert_eq!(
            sha256_digest(modified_content.as_bytes()),
            action.expected_new_sha256
        );
        assert!(modified_content.contains("id: legacy"));
        assert!(modified_content.contains("type: supersedes"));
        assert!(modified_content.contains("[old-note](./old-note.md)"));

        // Idempotency: re-applying the SAME plan produces 0 modifications
        let reapply_res = apply_plan(root, &plan, false).unwrap();
        assert_eq!(reapply_res.files_modified, 0);
        assert_eq!(reapply_res.files_already_up_to_date, 1);

        // Re-planning produces 0 actions
        let plan2 = plan_vault(root).unwrap();
        assert_eq!(plan2.actions.len(), 0);
    }

    #[test]
    fn test_apply_abort_on_hash_mismatch() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let note1 = "---\ntype: Concept\ntitle: Note 1\n---\n";
        let note2 = "---\ntype: Concept\ntitle: Note 2\n---\n";

        fs::write(root.join("n1.md"), note1).unwrap();
        fs::write(root.join("n2.md"), note2).unwrap();

        let plan = plan_vault(root).unwrap();
        assert_eq!(plan.actions.len(), 2);

        // Concurrently modify n2.md so its hash mismatches
        fs::write(root.join("n2.md"), "tampered content").unwrap();

        // Application must fail
        let err = apply_plan(root, &plan, false).unwrap_err();
        assert!(err.to_string().contains("Hash mismatch for file 'n2.md'"));

        // Note 1 MUST NOT have been modified (no partial mutation)
        let n1_content = fs::read_to_string(root.join("n1.md")).unwrap();
        assert_eq!(n1_content, note1);
    }
}
