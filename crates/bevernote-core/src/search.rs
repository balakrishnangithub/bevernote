use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params_from_iter, types::Value as SqlValue};

use crate::{
    db::Library,
    error::{BeverError, Result},
    models::{NoteFilter, NoteSort, NoteSummary},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Term {
    label: Option<String>,
    value: String,
    negated: bool,
    quoted: bool,
}

#[derive(Debug, Default)]
struct Compiled {
    predicates: Vec<String>,
    parameters: Vec<SqlValue>,
    has_text: bool,
}

impl Compiled {
    fn push(
        &mut self,
        mut sql: String,
        parameters: impl IntoIterator<Item = SqlValue>,
        negated: bool,
    ) {
        if negated {
            sql = format!("NOT ({sql})");
        }
        self.predicates.push(format!("({sql})"));
        self.parameters.extend(parameters);
    }
}

pub(crate) fn search_notes(library: &Library, filter: &NoteFilter) -> Result<Vec<NoteSummary>> {
    let mut compiled = compile_query(&filter.query)?;
    add_filter_predicates(&mut compiled, filter);
    let where_sql = if compiled.predicates.is_empty() {
        "1 = 1".to_owned()
    } else {
        compiled.predicates.join(" AND ")
    };
    let relevance = filter.sort == NoteSort::Relevance && compiled.has_text;
    let join = if relevance {
        "JOIN notes_fts ON notes_fts.note_id = n.id"
    } else {
        ""
    };
    let snippet = if relevance {
        "snippet(notes_fts, 2, '<mark>', '</mark>', '…', 20)"
    } else {
        "substr(n.plain_text, 1, 320)"
    };
    let order = match filter.sort {
        NoteSort::Relevance if relevance => {
            "bm25(notes_fts, 0.0, 8.0, 1.0, 2.0, 1.5), n.updated_at DESC"
        }
        NoteSort::Relevance | NoteSort::UpdatedDesc => "n.updated_at DESC",
        NoteSort::UpdatedAsc => "n.updated_at ASC",
        NoteSort::CreatedDesc => "n.created_at DESC",
        NoteSort::CreatedAsc => "n.created_at ASC",
        NoteSort::TitleAsc => "n.title COLLATE NOCASE ASC",
        NoteSort::TitleDesc => "n.title COLLATE NOCASE DESC",
    };
    let sql = format!(
        "SELECT n.id, n.notebook_id, nb.name, n.title, {snippet}, n.created_at, n.updated_at, n.deleted_at,
            COALESCE((SELECT group_concat(t.name, char(31)) FROM tags t JOIN note_tags nt ON nt.tag_id = t.id WHERE nt.note_id = n.id), ''),
            EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id),
            EXISTS(SELECT 1 FROM shortcuts s WHERE s.target_type = 'note' AND s.target_id = n.id)
         FROM notes n JOIN notebooks nb ON nb.id = n.notebook_id {join}
         WHERE {where_sql} ORDER BY {order} LIMIT ? OFFSET ?"
    );
    compiled
        .parameters
        .push(SqlValue::Integer(i64::from(filter.limit.clamp(1, 500))));
    compiled
        .parameters
        .push(SqlValue::Integer(i64::from(filter.offset)));
    let connection = library.connection.lock().expect("database mutex poisoned");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(compiled.parameters.iter()), |row| {
        let tags: String = row.get(8)?;
        Ok(NoteSummary {
            id: row.get(0)?,
            notebook_id: row.get(1)?,
            notebook_name: row.get(2)?,
            title: row.get(3)?,
            snippet: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
            deleted_at: row.get(7)?,
            tags: if tags.is_empty() {
                Vec::new()
            } else {
                tags.split(char::from(31)).map(str::to_owned).collect()
            },
            has_attachments: row.get(9)?,
            is_shortcut: row.get(10)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn add_filter_predicates(compiled: &mut Compiled, filter: &NoteFilter) {
    if filter.trash_only {
        compiled.predicates.push("n.deleted_at IS NOT NULL".into());
    } else if !filter.include_trash {
        compiled.predicates.push("n.deleted_at IS NULL".into());
    }
    if let Some(id) = &filter.notebook_id {
        compiled.push(
            "n.notebook_id = ?".into(),
            [SqlValue::Text(id.clone())],
            false,
        );
    }
    if let Some(id) = &filter.stack_id {
        compiled.push(
            "EXISTS(SELECT 1 FROM notebooks fnb WHERE fnb.id = n.notebook_id AND fnb.stack_id = ?)"
                .into(),
            [SqlValue::Text(id.clone())],
            false,
        );
    }
    for id in &filter.tag_ids {
        compiled.push(
            "EXISTS(SELECT 1 FROM note_tags fnt WHERE fnt.note_id = n.id AND fnt.tag_id = ?)"
                .into(),
            [SqlValue::Text(id.clone())],
            false,
        );
    }
    for (column, value) in [
        ("n.created_at", filter.created_from),
        ("n.updated_at", filter.updated_from),
    ] {
        if let Some(value) = value {
            compiled.push(format!("{column} >= ?"), [SqlValue::Integer(value)], false);
        }
    }
    for (column, value) in [
        ("n.created_at", filter.created_to),
        ("n.updated_at", filter.updated_to),
    ] {
        if let Some(value) = value {
            compiled.push(format!("{column} <= ?"), [SqlValue::Integer(value)], false);
        }
    }
    if let Some(contains) = &filter.contains {
        compiled.push(
            contains_predicate(contains).0,
            contains_predicate(contains).1,
            false,
        );
    }
}

fn compile_query(query: &str) -> Result<Compiled> {
    let terms = tokenize(query)?;
    let any = terms.first().is_some_and(|term| {
        term.label.as_deref() == Some("any") && term.value.is_empty() && !term.negated
    });
    let mut compiled = Compiled::default();
    let start = usize::from(any);
    let mut groups = Vec::new();
    for term in &terms[start..] {
        let mut part = Compiled::default();
        compile_term(term, &mut part)?;
        compiled.has_text |= part.has_text;
        if !part.predicates.is_empty() {
            groups.push((part.predicates.join(" AND "), part.parameters));
        }
    }
    if groups.is_empty() {
        return Ok(compiled);
    }
    let joiner = if any { " OR " } else { " AND " };
    compiled.predicates.push(
        groups
            .iter()
            .map(|(sql, _)| format!("({sql})"))
            .collect::<Vec<_>>()
            .join(joiner),
    );
    for (_, parameters) in groups {
        compiled.parameters.extend(parameters);
    }
    Ok(compiled)
}

fn compile_term(term: &Term, compiled: &mut Compiled) -> Result<()> {
    match term.label.as_deref() {
        None => {
            compiled.has_text = true;
            let value = fts_expression(&term.value, term.quoted, None);
            compiled.push(
                "n.id IN (SELECT note_id FROM notes_fts WHERE notes_fts MATCH ?)".into(),
                [SqlValue::Text(value)],
                term.negated,
            );
        }
        Some("intitle") => {
            compiled.has_text = true;
            compiled.push(
                "n.id IN (SELECT note_id FROM notes_fts WHERE notes_fts MATCH ?)".into(),
                [SqlValue::Text(fts_expression(
                    &term.value,
                    term.quoted,
                    Some("title"),
                ))],
                term.negated,
            );
        }
        Some("tag") => {
            let (sql, parameter) = name_predicate(
                "EXISTS(SELECT 1 FROM note_tags nt JOIN tags t ON t.id = nt.tag_id WHERE nt.note_id = n.id AND ",
                &term.value,
                "t.name",
            );
            compiled.push(sql, parameter, term.negated);
        }
        Some("notebook") => {
            let (sql, parameter) = name_predicate(
                "EXISTS(SELECT 1 FROM notebooks nb2 WHERE nb2.id = n.notebook_id AND ",
                &term.value,
                "nb2.name",
            );
            compiled.push(sql, parameter, term.negated);
        }
        Some("stack") => {
            let (sql, parameter) = name_predicate(
                "EXISTS(SELECT 1 FROM notebooks nb2 JOIN stacks st ON st.id = nb2.stack_id WHERE nb2.id = n.notebook_id AND ",
                &term.value,
                "st.name",
            );
            compiled.push(sql, parameter, term.negated);
        }
        Some("created" | "updated") => {
            let timestamp = parse_search_date(&term.value)?;
            compiled.push(
                format!("n.{}_at >= ?", term.label.as_deref().unwrap_or("created")),
                [SqlValue::Integer(timestamp)],
                term.negated,
            );
        }
        Some("resource") => {
            compiled.push(
                "EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id AND r.mime LIKE ?)".into(),
                [SqlValue::Text(term.value.replace('*', "%"))],
                term.negated,
            );
        }
        Some("todo") => {
            let sql = match term.value.as_str() {
                "true" => "n.content_html LIKE '%<en-todo%' AND (n.content_html LIKE '%checked=\"false\"%' OR n.content_html LIKE '%data-checked=\"false\"%' OR n.content_html LIKE '%<en-todo></en-todo>%' OR n.content_html LIKE '%<en-todo />%')",
                "false" => "n.content_html LIKE '%checked=\"true\"%' OR n.content_html LIKE '%data-checked=\"true\"%'",
                "*" => "n.content_html LIKE '%<en-todo%' OR n.content_html LIKE '%data-type=\"taskItem\"%'",
                _ => return Err(BeverError::Invalid("todo: expects true, false, or *".into())),
            };
            compiled.push(sql.into(), [], term.negated);
        }
        Some("encryption") => {
            compiled.push("n.content_html LIKE '%<en-crypt%'".into(), [], term.negated);
        }
        Some("contains") => {
            let (sql, parameters) = contains_predicate(&term.value);
            compiled.push(sql, parameters, term.negated);
        }
        Some("source" | "sourceurl") => {
            compiled.push(
                "COALESCE(json_extract(n.attributes_json, '$.source-url'), '') LIKE ?".into(),
                [SqlValue::Text(term.value.replace('*', "%"))],
                term.negated,
            );
        }
        Some("any") => {
            return Err(BeverError::Invalid(
                "any: is only valid at the start of a search".into(),
            ));
        }
        Some(label) => {
            return Err(BeverError::Invalid(format!(
                "unsupported search operator: {label}:"
            )));
        }
    }
    Ok(())
}

fn contains_predicate(value: &str) -> (String, Vec<SqlValue>) {
    match value {
        "images" | "image" => (
            "EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id AND r.mime LIKE 'image/%')".into(),
            vec![],
        ),
        "audio" => (
            "EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id AND r.mime LIKE 'audio/%')".into(),
            vec![],
        ),
        "video" => (
            "EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id AND r.mime LIKE 'video/%')".into(),
            vec![],
        ),
        "pdf" => (
            "EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id AND r.mime = 'application/pdf')".into(),
            vec![],
        ),
        "files" | "attachments" => (
            "EXISTS(SELECT 1 FROM resources r WHERE r.note_id = n.id)".into(),
            vec![],
        ),
        "checklist" | "todo" => (
            "n.content_html LIKE '%<en-todo%' OR n.content_html LIKE '%data-type=\"taskItem\"%'".into(),
            vec![],
        ),
        "code" | "codeblocks" => ("n.content_html LIKE '%<pre%'".into(), vec![]),
        "tables" | "table" => ("n.content_html LIKE '%<table%'".into(), vec![]),
        "links" | "link" => ("n.content_html LIKE '%<a %'".into(), vec![]),
        _ => ("0 = 1".into(), vec![]),
    }
}

fn name_predicate(prefix: &str, value: &str, column: &str) -> (String, Vec<SqlValue>) {
    if value == "*" {
        return (format!("{prefix}1 = 1)"), vec![]);
    }
    if value.contains('*') {
        (
            format!("{prefix}{column} LIKE ? COLLATE NOCASE)"),
            vec![SqlValue::Text(value.replace('*', "%"))],
        )
    } else {
        (
            format!("{prefix}{column} = ? COLLATE NOCASE)"),
            vec![SqlValue::Text(value.to_owned())],
        )
    }
}

fn fts_expression(value: &str, quoted: bool, column: Option<&str>) -> String {
    let prefix = if !quoted && value.ends_with('*') && value.len() > 1 {
        "*"
    } else {
        ""
    };
    let value = value.trim_end_matches('*').replace('"', "\"\"");
    let expression = format!("\"{value}\"{prefix}");
    column.map_or(expression.clone(), |column| {
        format!("{column}:{expression}")
    })
}

fn tokenize(query: &str) -> Result<Vec<Term>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut escaped = false;
    for character in query.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '"' {
            quoted = !quoted;
            was_quoted = true;
            continue;
        }
        if character.is_whitespace() && !quoted {
            if !current.is_empty() {
                tokens.push(make_term(std::mem::take(&mut current), was_quoted));
                was_quoted = false;
            }
            continue;
        }
        current.push(character);
    }
    if quoted || escaped {
        return Err(BeverError::Invalid(
            "unterminated quote or escape in search".into(),
        ));
    }
    if !current.is_empty() {
        tokens.push(make_term(current, was_quoted));
    }
    Ok(tokens)
}

fn make_term(mut raw: String, quoted: bool) -> Term {
    let negated = raw.starts_with('-') && raw.len() > 1;
    if negated {
        raw.remove(0);
    }
    let (label, value) = raw
        .split_once(':')
        .map_or((None, raw.as_str()), |(label, value)| {
            (Some(label.to_lowercase()), value)
        });
    Term {
        label,
        value: value.to_owned(),
        negated,
        quoted,
    }
}

fn parse_search_date(value: &str) -> Result<i64> {
    let now = Utc::now();
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y%m%d") {
        return Ok(date
            .and_hms_opt(0, 0, 0)
            .expect("midnight exists")
            .and_utc()
            .timestamp_millis());
    }
    if let Ok(date) = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ") {
        return Ok(date.and_utc().timestamp_millis());
    }
    let (unit, amount) = value.split_once('-').map_or((value, 0), |(unit, amount)| {
        (unit, amount.parse::<i64>().unwrap_or(0))
    });
    let date = match unit {
        "day" => now - Duration::days(amount),
        "week" => {
            let start =
                now.date_naive() - Duration::days(i64::from(now.weekday().num_days_from_monday()));
            Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).expect("midnight exists"))
                - Duration::weeks(amount)
        }
        "month" => {
            let start = now.date_naive().with_day(1).expect("first day exists");
            let mut shifted =
                Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).expect("midnight exists"));
            for _ in 0..amount {
                shifted = shifted - Duration::days(shifted.day().into());
            }
            shifted
        }
        "year" => {
            let year = now.year() - i32::try_from(amount).unwrap_or(0);
            Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0)
                .single()
                .ok_or_else(|| BeverError::Invalid("invalid year in search".into()))?
        }
        _ => return Err(BeverError::Invalid(format!("invalid date search: {value}"))),
    };
    Ok(date.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::models::{NoteFilter, SaveNote};

    #[test]
    fn tokenizes_quoted_operators_and_negation() {
        let terms =
            tokenize("notebook:\"My Notebook\" -tag:archive \"exact phrase\" prefix*").unwrap();
        assert_eq!(terms.len(), 4);
        assert_eq!(terms[0].value, "My Notebook");
        assert!(terms[1].negated);
        assert!(terms[2].quoted);
    }

    #[test]
    fn searches_title_body_tags_and_notebooks_offline() {
        let directory = tempfile::tempdir().unwrap();
        let library =
            Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
        let notebook = library.ensure_notebook("Research", None).unwrap();
        let note = library.create_note(Some(&notebook)).unwrap();
        library
            .save_note(&SaveNote {
                id: note.id,
                notebook_id: notebook,
                title: "Offline Search".into(),
                content_html: "<p>Fast local indexing</p>".into(),
                attributes: json!({}),
                tags: vec!["engineering".into()],
            })
            .unwrap();
        for query in [
            "intitle:offline",
            "indexing",
            "tag:engineering",
            "notebook:Research",
            "any: missing indexing",
        ] {
            let results = library
                .list_notes(&NoteFilter {
                    query: query.into(),
                    ..NoteFilter::default()
                })
                .unwrap();
            assert_eq!(results.len(), 1, "query: {query}");
        }
        let results = library
            .list_notes(&NoteFilter {
                query: "-tag:engineering".into(),
                ..NoteFilter::default()
            })
            .unwrap();
        assert!(results.is_empty());
    }
}
