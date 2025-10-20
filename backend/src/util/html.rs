/// Very small HTML cleaner to remove tags and common noise from feed summaries.
/// - Removes entire <script> and <style> blocks (case-insensitive)
/// - Strips other tags like <p>, <br>, etc.
/// - Collapses excessive whitespace and trims ends
pub fn strip_html_basic(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Remove <script>...</script> and <style>...</style> blocks (case-insensitive)
    let mut buf = input.to_string();
    for tag in ["script", "style"] {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        loop {
            // find case-insensitively
            let lower = buf.to_lowercase();
            if let Some(start) = lower.find(&open) {
                if let Some(end_rel) = lower[start..].find(&close) {
                    let end = start + end_rel + close.len();
                    buf.replace_range(start..end, "");
                    continue;
                } else {
                    // no closing tag; drop from start to end
                    buf.replace_range(start..buf.len(), "");
                }
            }
            break;
        }
    }

    // Strip remaining tags by skipping characters between '<' and '>'
    let mut out = String::with_capacity(buf.len());
    let mut in_tag = false;
    for ch in buf.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    // Collapse whitespace
    let mut collapsed = String::with_capacity(out.len());
    let mut last_space = false;
    for ch in out.chars() {
        if ch.is_whitespace() {
            if !last_space {
                collapsed.push(' ');
                last_space = true;
            }
        } else {
            collapsed.push(ch);
            last_space = false;
        }
    }

    collapsed.trim().to_string()
}

