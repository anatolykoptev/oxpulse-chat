use sqlx::PgPool;

/// Ordered list of embedded migrations.
///
/// Each entry is `(name, sql)`. The name is used as the idempotency key in
/// `schema_migrations`. SQL is split by `split_statements` which handles
/// `--` line comments, `/* */` block comments, `'…'` / `"…"` string literals
/// (with doubled-quote escape), and `$tag$…$tag$` dollar-quoted blocks so that
/// semicolons inside any of those constructs are never treated as terminators.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_analytics.sql",
        include_str!("../migrations/001_analytics.sql"),
    ),
    (
        "20260417_partner_tokens.sql",
        include_str!("../migrations/20260417_partner_tokens.sql"),
    ),
    (
        "20260418_partner_nodes.sql",
        include_str!("../migrations/20260418_partner_nodes.sql"),
    ),
];

/// Split a SQL string into individual statements, respecting:
/// - `--` line comments (stripped, not a terminator)
/// - `/* … */` block comments (stripped, non-nested)
/// - `'…'` and `"…"` quoted literals with doubled-quote escape (`''`, `""`)
/// - `$tag$…$tag$` Postgres dollar-quoted strings (tag may be empty)
///
/// Returns trimmed, non-empty statements.
fn split_statements(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();

    let mut statements: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // --- line comment: -- ... \n
        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            // skip until newline (don't add to current)
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            // consume the newline itself
            if i < len {
                i += 1;
            }
            continue;
        }

        // --- block comment: /* ... */
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i < len {
                if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // --- dollar-quoted string: $tag$...$tag$
        // A dollar-quote starts with `$`, optionally an identifier, then `$`.
        // Identifier chars: [A-Za-z_][A-Za-z0-9_]*  (may be empty → $$)
        if ch == '$' {
            // Try to parse a dollar-quote opening tag.
            let tag_start = i + 1;
            let mut j = tag_start;
            // First char: letter or underscore (or immediately closing $)
            if j < len && (chars[j].is_ascii_alphabetic() || chars[j] == '_') {
                j += 1;
                while j < len && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
            }
            if j < len && chars[j] == '$' {
                // Valid dollar-quote opening: chars[i..=j]
                let tag: String = chars[tag_start..j].iter().collect();
                let open_len = j - i + 1; // length of $tag$
                let body_start = j + 1;
                let closing: Vec<char> = format!("${}$", tag).chars().collect();
                let clen = closing.len();

                // Find the matching closing $tag$
                let mut k = body_start;
                let mut found_close = false;
                while k + clen <= len {
                    if chars[k..k + clen] == closing[..] {
                        // Emit the whole dollar-quoted block verbatim (including delimiters)
                        let block: String = chars[i..k + clen].iter().collect();
                        current.push_str(&block);
                        i = k + clen;
                        found_close = true;
                        break;
                    }
                    k += 1;
                }
                if found_close {
                    continue;
                }
                // No closing tag found — treat the opening $ as literal
                let literal: String = chars[i..i + open_len].iter().collect();
                current.push_str(&literal);
                i += open_len;
                continue;
            }
            // Not a valid dollar-quote, fall through to push ch
        }

        // --- single-quoted string: '...' with '' escape
        if ch == '\'' {
            current.push(ch);
            i += 1;
            while i < len {
                let c = chars[i];
                if c == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        // escaped quote
                        current.push('\'');
                        current.push('\'');
                        i += 2;
                    } else {
                        current.push('\'');
                        i += 1;
                        break;
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
            }
            continue;
        }

        // --- double-quoted identifier: "..." with "" escape
        if ch == '"' {
            current.push(ch);
            i += 1;
            while i < len {
                let c = chars[i];
                if c == '"' {
                    if i + 1 < len && chars[i + 1] == '"' {
                        current.push('"');
                        current.push('"');
                        i += 2;
                    } else {
                        current.push('"');
                        i += 1;
                        break;
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
            }
            continue;
        }

        // --- statement terminator
        if ch == ';' {
            let stmt = current.trim().to_string();
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current = String::new();
            i += 1;
            continue;
        }

        // default: push character
        current.push(ch);
        i += 1;
    }

    // trailing statement without semicolon
    let stmt = current.trim().to_string();
    if !stmt.is_empty() {
        statements.push(stmt);
    }

    statements
}

pub async fn run(pool: &PgPool) {
    // 1. Ensure the tracker table exists.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations ( \
            name TEXT PRIMARY KEY, \
            applied_at TIMESTAMPTZ DEFAULT NOW() \
        )",
    )
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("failed to create schema_migrations: {e}"));

    // 2. Seed existing migrations so they are never re-applied after this
    //    change is first deployed onto a DB that already ran them.
    sqlx::query(
        "INSERT INTO schema_migrations (name) \
         VALUES ('001_analytics.sql'), ('20260417_partner_tokens.sql') \
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("failed to seed schema_migrations: {e}"));

    // 3. Apply each migration that hasn't been recorded yet.
    let mut applied = 0usize;
    for (name, sql) in MIGRATIONS {
        let already_applied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = $1)")
                .bind(name)
                .fetch_one(pool)
                .await
                .unwrap_or_else(|e| panic!("schema_migrations check for {name} failed: {e}"));

        if already_applied {
            tracing::debug!(migration = %name, "migration already applied, skipping");
            continue;
        }

        for statement in split_statements(sql) {
            let stmt = statement.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt)
                .execute(pool)
                .await
                .unwrap_or_else(|e| panic!("migration {name} failed on statement: {stmt}\n{e}"));
        }

        sqlx::query("INSERT INTO schema_migrations (name) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(name)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("failed to record migration {name}: {e}"));

        tracing::debug!(migration = %name, "migration applied");
        applied += 1;
    }

    tracing::info!(applied, total = MIGRATIONS.len(), "migrations complete");
}

#[cfg(test)]
mod tests {
    use super::split_statements;

    #[test]
    fn basic_split() {
        let stmts = split_statements("SELECT 1; SELECT 2");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn trailing_semicolon() {
        let stmts = split_statements("SELECT 1;");
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1");
    }

    #[test]
    fn no_trailing_semicolon() {
        let stmts = split_statements("SELECT 1");
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1");
    }

    #[test]
    fn line_comment_with_semicolon() {
        let sql = "-- bootstrap token; updated on each\nCREATE TABLE x (id INT);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1, "got: {:?}", stmts);
        assert!(stmts[0].starts_with("CREATE TABLE"), "got: {}", stmts[0]);
    }

    #[test]
    fn block_comment_with_semicolon() {
        let stmts = split_statements("/* a; b */ SELECT 1;");
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 1");
    }

    #[test]
    fn string_with_semicolon() {
        let stmts = split_statements("SELECT ';' FROM t; SELECT 2");
        assert_eq!(stmts.len(), 2, "got: {:?}", stmts);
        assert!(stmts[0].contains("';'"), "got: {}", stmts[0]);
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn escaped_quote_in_string() {
        let stmts = split_statements("SELECT 'it''s' FROM t");
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("'it''s'"), "got: {}", stmts[0]);
    }

    #[test]
    fn identifier_with_semicolon() {
        let stmts = split_statements(r#"SELECT "x;y" FROM t"#);
        assert_eq!(stmts.len(), 1, "got: {:?}", stmts);
        assert!(stmts[0].contains(r#""x;y""#), "got: {}", stmts[0]);
    }

    #[test]
    fn dollar_quote_anonymous() {
        let sql = "DO $$ BEGIN RAISE NOTICE 'hi;'; END $$; SELECT 1";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2, "got: {:?}", stmts);
        assert!(stmts[0].starts_with("DO $$"), "got: {}", stmts[0]);
        assert_eq!(stmts[1], "SELECT 1");
    }

    #[test]
    fn dollar_quote_tagged() {
        let sql = "DO $foo$ a; b; $foo$; SELECT 2";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2, "got: {:?}", stmts);
        assert!(stmts[0].starts_with("DO $foo$"), "got: {}", stmts[0]);
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn empty_input() {
        let stmts = split_statements("");
        assert_eq!(stmts, Vec::<String>::new());
    }

    #[test]
    fn only_whitespace_and_comments() {
        let stmts = split_statements("-- hello\n/* world */");
        assert_eq!(stmts, Vec::<String>::new(), "got: {:?}", stmts);
    }

    #[test]
    fn three_day_interval_regression() {
        // comment containing a quote character must NOT open a string literal
        let sql = "-- e.g. interval '30 days'\nCREATE INDEX foo ON bar (baz);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1, "got: {:?}", stmts);
        assert!(stmts[0].starts_with("CREATE INDEX"), "got: {}", stmts[0]);
    }
}
