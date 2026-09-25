//! CSV in and out, for import and export. RFC 4180: quoted fields may hold
//! the delimiter, doubled quotes and line breaks. Small enough that a
//! dependency would be more to audit than to write.

/// Parse a whole CSV text into rows of fields.
pub fn parse(text: &str, delimiter: char) -> Result<Vec<Vec<String>>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut at_field_start = true;
    let mut chars = text.chars().peekable();
    let mut line = 1;

    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                '\n' => {
                    line += 1;
                    field.push(c);
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if at_field_start => {
                quoted = true;
                at_field_start = false;
            }
            c if c == delimiter => {
                row.push(std::mem::take(&mut field));
                at_field_start = true;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                line += 1;
                row.push(std::mem::take(&mut field));
                if !(row.len() == 1 && row[0].is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
                at_field_start = true;
            }
            _ => {
                field.push(c);
                at_field_start = false;
            }
        }
    }
    if quoted {
        return Err(format!("A quoted field that starts before line {line} never ends"));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

/// One CSV line, with a trailing newline. `None` becomes an empty field.
pub fn line(fields: &[Option<String>], delimiter: char) -> String {
    let mut out = fields
        .iter()
        .map(|f| {
            let f = f.as_deref().unwrap_or("");
            if f.contains(delimiter) || f.contains('"') || f.contains('\n') || f.contains('\r') || f.starts_with(' ') || f.ends_with(' ') {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(&delimiter.to_string());
    out.push_str("\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_keep_delimiters_quotes_and_newlines() {
        let rows = parse("id,note\r\n1,\"a, b\"\n2,\"say \"\"hi\"\"\"\n3,\"two\nlines\"\n", ',').unwrap();
        assert_eq!(rows, vec![
            vec!["id", "note"],
            vec!["1", "a, b"],
            vec!["2", "say \"hi\""],
            vec!["3", "two\nlines"],
        ]);
    }

    #[test]
    fn a_missing_final_newline_and_a_bom_are_fine() {
        assert_eq!(parse("\u{feff}a;b\n1;", ';').unwrap(), vec![vec!["a", "b"], vec!["1", ""]]);
    }

    #[test]
    fn an_unterminated_quote_is_an_error() {
        assert!(parse("a\n\"open", ',').is_err());
    }

    #[test]
    fn writing_round_trips() {
        let fields = vec![Some("plain".to_string()), Some("a,b".into()), None, Some("q\"x".into())];
        let text = line(&fields, ',');
        assert_eq!(text, "plain,\"a,b\",,\"q\"\"x\"\r\n");
        assert_eq!(parse(&text, ',').unwrap()[0], vec!["plain", "a,b", "", "q\"x"]);
    }
}
