use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, NaiveDateTime, Utc};
use quick_xml::{events::Event, Reader};
use serde_json::{Map, Value};

use crate::{
    db::Library,
    enml::{enml_to_html, escape_cdata, html_to_enml},
    error::{BeverError, Result},
    models::{ImportProgress, ImportReport, Note},
    ocr::LocalOcr,
    repository::ImportNoteResult,
};

#[derive(Debug, Clone)]
pub struct EnexResource {
    pub data: Vec<u8>,
    pub alternate_data: Option<Vec<u8>>,
    pub mime: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<u64>,
    pub recognition_xml: Option<String>,
    pub ocr_text: Option<String>,
    pub attributes: Value,
}

#[derive(Debug, Clone)]
pub struct EnexNote {
    pub title: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub tags: Vec<String>,
    pub attributes: Value,
    pub evernote_guid: Option<String>,
    pub resources: Vec<EnexResource>,
}

#[derive(Debug, Default)]
struct ResourceBuilder {
    data: String,
    alternate_data: String,
    mime: String,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<u64>,
    recognition_xml: String,
    attributes: Map<String, Value>,
}

impl ResourceBuilder {
    fn build(self) -> Result<EnexResource> {
        let data = decode_base64(&self.data)?;
        let alternate_data = if self.alternate_data.trim().is_empty() {
            None
        } else {
            Some(decode_base64(&self.alternate_data)?)
        };
        let recognition_xml = nonempty(self.recognition_xml);
        Ok(EnexResource {
            data,
            alternate_data,
            mime: if self.mime.is_empty() {
                "application/octet-stream".into()
            } else {
                self.mime
            },
            width: self.width,
            height: self.height,
            duration: self.duration,
            recognition_xml,
            ocr_text: None,
            attributes: Value::Object(self.attributes),
        })
    }
}

#[derive(Debug)]
struct NoteBuilder {
    title: String,
    content: String,
    created: String,
    updated: String,
    tags: Vec<String>,
    attributes: Map<String, Value>,
    evernote_guid: Option<String>,
    resources: Vec<EnexResource>,
}

impl Default for NoteBuilder {
    fn default() -> Self {
        Self {
            title: String::new(),
            content: String::new(),
            created: String::new(),
            updated: String::new(),
            tags: Vec::new(),
            attributes: Map::new(),
            evernote_guid: None,
            resources: Vec::new(),
        }
    }
}

impl NoteBuilder {
    fn build(self) -> Result<EnexNote> {
        let created_at =
            parse_enex_date(&self.created).unwrap_or_else(|_| Utc::now().timestamp_millis());
        let updated_at = if self.updated.is_empty() {
            created_at
        } else {
            parse_enex_date(&self.updated).unwrap_or(created_at)
        };
        Ok(EnexNote {
            title: self.title,
            content: self.content,
            created_at,
            updated_at,
            tags: self.tags,
            attributes: Value::Object(self.attributes),
            evernote_guid: self.evernote_guid,
            resources: self.resources,
        })
    }
}

pub struct EnexReader<R: BufRead> {
    reader: Reader<R>,
    buffer: Vec<u8>,
    path: Vec<String>,
    note: Option<NoteBuilder>,
    resource: Option<ResourceBuilder>,
    finished: bool,
}

impl<R: BufRead> EnexReader<R> {
    pub fn new(reader: R) -> Self {
        let mut reader = Reader::from_reader(reader);
        reader.config_mut().trim_text(false);
        reader.config_mut().check_end_names = true;
        Self {
            reader,
            buffer: Vec::new(),
            path: Vec::new(),
            note: None,
            resource: None,
            finished: false,
        }
    }

    pub fn next_note(&mut self) -> Result<Option<EnexNote>> {
        self.next_note_with_progress(|_| {})
    }

    pub fn next_note_with_progress(
        &mut self,
        mut on_progress: impl FnMut(u64),
    ) -> Result<Option<EnexNote>> {
        if self.finished {
            return Ok(None);
        }
        let mut last_reported = 0;
        loop {
            let position = self.reader.buffer_position();
            if position >= last_reported + 16 * 1024 * 1024 {
                on_progress(position);
                last_reported = position;
            }
            match self.reader.read_event_into(&mut self.buffer)? {
                Event::Start(event) => {
                    let name = xml_name(event.name().as_ref());
                    if name == "note" && self.note.is_none() {
                        self.note = Some(NoteBuilder::default());
                    } else if name == "resource" && self.note.is_some() {
                        self.resource = Some(ResourceBuilder::default());
                    }
                    self.path.push(name);
                }
                Event::Text(event) => {
                    let raw = String::from_utf8_lossy(event.as_ref());
                    let decoded = quick_xml::escape::unescape(&raw)
                        .map_err(|error| BeverError::Invalid(error.to_string()))?
                        .into_owned();
                    self.handle_text(&decoded)?;
                }
                Event::GeneralRef(event) => {
                    let name = String::from_utf8_lossy(event.as_ref()).into_owned();
                    let decoded = match name.as_str() {
                        "amp" => "&".to_owned(),
                        "lt" => "<".to_owned(),
                        "gt" => ">".to_owned(),
                        "quot" => "\"".to_owned(),
                        "apos" => "'".to_owned(),
                        _ if name.starts_with("#x") => u32::from_str_radix(&name[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(String::from)
                            .ok_or_else(|| BeverError::Invalid("invalid XML entity".into()))?,
                        _ if name.starts_with('#') => name[1..]
                            .parse::<u32>()
                            .ok()
                            .and_then(char::from_u32)
                            .map(String::from)
                            .ok_or_else(|| BeverError::Invalid("invalid XML entity".into()))?,
                        _ => format!("&{name};"),
                    };
                    self.handle_text(&decoded)?;
                }
                Event::CData(event) => {
                    let decoded = event
                        .decode()
                        .map_err(|error| BeverError::Invalid(error.to_string()))?
                        .into_owned();
                    self.handle_text(&decoded)?;
                }
                Event::End(event) => {
                    let name = xml_name(event.name().as_ref());
                    if name == "resource" {
                        if let (Some(note), Some(resource)) =
                            (self.note.as_mut(), self.resource.take())
                        {
                            note.resources.push(resource.build()?);
                        }
                    }
                    self.path.pop();
                    if name == "note" {
                        let note = self
                            .note
                            .take()
                            .ok_or_else(|| BeverError::Invalid("unexpected </note>".into()))?
                            .build()?;
                        self.buffer.clear();
                        return Ok(Some(note));
                    }
                }
                Event::Eof => {
                    self.finished = true;
                    self.buffer.clear();
                    return Ok(None);
                }
                _ => {}
            }
            self.buffer.clear();
        }
    }

    fn handle_text(&mut self, text: &str) -> Result<()> {
        let Some(note) = self.note.as_mut() else {
            return Ok(());
        };
        let leaf = self.path.last().map_or("", String::as_str);
        if let Some(resource) = self.resource.as_mut() {
            match leaf {
                "data" => resource.data.push_str(text),
                "alternate-data" => resource.alternate_data.push_str(text),
                "mime" => resource.mime.push_str(text),
                "width" => resource.width = text.trim().parse().ok(),
                "height" => resource.height = text.trim().parse().ok(),
                "duration" => resource.duration = text.trim().parse().ok(),
                "recognition" => resource.recognition_xml.push_str(text),
                _ if self.path.iter().any(|part| part == "resource-attributes") => {
                    insert_attribute(&mut resource.attributes, leaf, text);
                }
                _ => {}
            }
            return Ok(());
        }
        match leaf {
            "title" => note.title.push_str(text),
            "content" => note.content.push_str(text),
            "created" => note.created.push_str(text),
            "updated" => note.updated.push_str(text),
            "tag" => note.tags.push(text.trim().to_owned()),
            "guid" => note.evernote_guid = nonempty(text.trim().to_owned()),
            _ if self.path.iter().any(|part| part == "note-attributes") => {
                insert_attribute(&mut note.attributes, leaf, text);
            }
            _ => {}
        }
        Ok(())
    }
}

fn insert_attribute(attributes: &mut Map<String, Value>, key: &str, value: &str) {
    if key.is_empty() || matches!(key, "note-attributes" | "resource-attributes") {
        return;
    }
    attributes
        .entry(key.to_owned())
        .and_modify(|existing| match existing {
            Value::Array(values) => values.push(Value::String(value.to_owned())),
            _ => {
                *existing = Value::Array(vec![existing.take(), Value::String(value.to_owned())]);
            }
        })
        .or_insert_with(|| Value::String(value.to_owned()));
}

fn xml_name(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    BASE64
        .decode(
            value
                .bytes()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| BeverError::Invalid(format!("invalid resource base64: {error}")))
}

pub fn parse_enex_date(value: &str) -> Result<i64> {
    let parsed = NaiveDateTime::parse_from_str(value.trim(), "%Y%m%dT%H%M%SZ")
        .map_err(|error| BeverError::Invalid(format!("invalid ENEX date {value:?}: {error}")))?;
    Ok(parsed.and_utc().timestamp_millis())
}

pub fn import_enex(
    library: &Library,
    path: impl AsRef<Path>,
    notebook_name: Option<&str>,
) -> Result<ImportReport> {
    import_enex_with_progress(library, path, notebook_name, |_| {})
}

pub fn import_enex_with_progress(
    library: &Library,
    path: impl AsRef<Path>,
    notebook_name: Option<&str>,
    mut progress: impl FnMut(ImportProgress),
) -> Result<ImportReport> {
    let path = path.as_ref();
    let total_bytes = path.metadata()?.len();
    let fallback_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Imported notes");
    let mut report = ImportReport::default();
    emit_progress(&mut progress, "preparing", &report, 0, None, 0, total_bytes);
    let notebook_id = library.ensure_notebook(notebook_name.unwrap_or(fallback_name), None)?;
    report.notebook_id = notebook_id.clone();
    emit_progress(
        &mut progress,
        "notebookReady",
        &report,
        0,
        None,
        0,
        total_bytes,
    );
    let file = File::open(path)?;
    let mut reader = EnexReader::new(BufReader::with_capacity(1024 * 1024, file));
    let ocr_enabled = library.setting("ocr_enabled")?.as_deref() != Some("false");
    let mut ocr = if ocr_enabled {
        match LocalOcr::new() {
            Ok(engine) => Some(engine),
            Err(error) => {
                report
                    .warnings
                    .push(format!("Local OCR unavailable: {error}"));
                None
            }
        }
    } else {
        None
    };
    let mut note_index = 0;
    loop {
        let next = reader.next_note_with_progress(|bytes_read| {
            emit_progress(
                &mut progress,
                "reading",
                &report,
                note_index + 1,
                None,
                bytes_read,
                total_bytes,
            );
        })?;
        let Some(mut note) = next else {
            break;
        };
        note_index += 1;
        emit_progress(
            &mut progress,
            "processing",
            &report,
            note_index,
            Some(&note.title),
            reader.reader.buffer_position(),
            total_bytes,
        );
        if let Some(engine) = ocr.as_mut() {
            for resource in &mut note.resources {
                if resource.recognition_xml.is_none()
                    && matches!(
                        resource.mime.as_str(),
                        "image/png" | "image/jpeg" | "image/webp"
                    )
                {
                    match engine.recognize(&resource.data) {
                        Ok(text) => resource.ocr_text = text,
                        Err(_)
                            if !report
                                .warnings
                                .iter()
                                .any(|warning| warning.starts_with("Some images")) =>
                        {
                            report.warnings.push(
                                "Some images could not be OCR-indexed; their notes and attachments were imported unchanged.".into(),
                            );
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        match library.import_enex_note(note, &notebook_id)? {
            ImportNoteResult::Imported(note) => {
                report.notes_imported += 1;
                report.resources_imported += note.resources.len() as u64;
                report.bytes_imported += note.resources.iter().map(|item| item.size).sum::<u64>();
            }
            ImportNoteResult::Duplicate(_) => report.notes_skipped += 1,
        }
        emit_progress(
            &mut progress,
            "noteImported",
            &report,
            note_index,
            None,
            reader.reader.buffer_position(),
            total_bytes,
        );
    }
    emit_progress(
        &mut progress,
        "finalizing",
        &report,
        note_index,
        None,
        total_bytes,
        total_bytes,
    );
    library.rewrite_import_links()?;
    library.optimize()?;
    emit_progress(
        &mut progress,
        "complete",
        &report,
        note_index,
        None,
        total_bytes,
        total_bytes,
    );
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    progress: &mut impl FnMut(ImportProgress),
    phase: &str,
    report: &ImportReport,
    note_index: u64,
    note_title: Option<&str>,
    bytes_read: u64,
    total_bytes: u64,
) {
    progress(ImportProgress {
        phase: phase.to_owned(),
        notebook_id: report.notebook_id.clone(),
        note_index,
        note_title: note_title.map(str::to_owned),
        notes_imported: report.notes_imported,
        notes_skipped: report.notes_skipped,
        resources_imported: report.resources_imported,
        bytes_read,
        total_bytes,
    });
}

pub fn export_enex(library: &Library, notes: &[Note], destination: impl AsRef<Path>) -> Result<()> {
    let file = File::create(destination)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE en-export SYSTEM \"http://xml.evernote.com/pub/evernote-export4.dtd\">\n")?;
    write!(
        writer,
        "<en-export export-date=\"{}\" application=\"Bevernote\" version=\"0.1.2\">",
        format_enex_date(Utc::now().timestamp_millis())
    )?;
    for note in notes {
        write!(writer, "<note><title>{}</title>", xml(&note.title))?;
        if let Some(guid) = &note.evernote_guid {
            write!(writer, "<guid>{}</guid>", xml(guid))?;
        }
        write!(
            writer,
            "<created>{}</created><updated>{}</updated>",
            format_enex_date(note.created_at),
            format_enex_date(note.updated_at)
        )?;
        let original_html = note.original_enml.as_deref().map(enml_to_html);
        let enml = match (&note.original_enml, original_html) {
            (Some(original), Some(html)) if html == note.content_html => original.clone(),
            _ => html_to_enml(&note.content_html),
        };
        write!(
            writer,
            "<content><![CDATA[{}]]></content>",
            escape_cdata(&enml)
        )?;
        for tag in &note.tags {
            write!(writer, "<tag>{}</tag>", xml(tag))?;
        }
        write_attributes(&mut writer, "note-attributes", &note.attributes)?;
        for resource in &note.resources {
            let bytes = library.read_blob(&resource.sha256)?;
            write!(
                writer,
                "<resource><data encoding=\"base64\">{}</data><mime>{}</mime>",
                BASE64.encode(bytes),
                xml(&resource.mime)
            )?;
            if let Some(width) = resource.width {
                write!(writer, "<width>{width}</width>")?;
            }
            if let Some(height) = resource.height {
                write!(writer, "<height>{height}</height>")?;
            }
            if let Some(duration) = resource.duration {
                write!(writer, "<duration>{duration}</duration>")?;
            }
            if let Some(recognition) = &resource.recognition_xml {
                write!(
                    writer,
                    "<recognition><![CDATA[{}]]></recognition>",
                    escape_cdata(recognition)
                )?;
            }
            if let Some(alternate) = &resource.alternate_sha256 {
                let bytes = library.read_blob(alternate)?;
                write!(
                    writer,
                    "<alternate-data encoding=\"base64\">{}</alternate-data>",
                    BASE64.encode(bytes)
                )?;
            }
            write_attributes(&mut writer, "resource-attributes", &resource.attributes)?;
            writer.write_all(b"</resource>")?;
        }
        writer.write_all(b"</note>")?;
    }
    writer.write_all(b"</en-export>")?;
    writer.flush()?;
    Ok(())
}

fn write_attributes(writer: &mut impl Write, container: &str, attributes: &Value) -> Result<()> {
    let Some(values) = attributes.as_object() else {
        return Ok(());
    };
    write!(writer, "<{container}>")?;
    for (key, value) in values {
        if !valid_xml_name(key) {
            continue;
        }
        match value {
            Value::Array(items) => {
                for item in items {
                    write!(writer, "<{key}>{}</{key}>", xml(&value_text(item)))?;
                }
            }
            _ => write!(writer, "<{key}>{}</{key}>", xml(&value_text(value)))?,
        }
    }
    write!(writer, "</{container}>")?;
    Ok(())
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn valid_xml_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn xml(value: &str) -> String {
    html_escape::encode_text(value).into_owned()
}

fn format_enex_date(timestamp: i64) -> String {
    DateTime::from_timestamp_millis(timestamp)
        .unwrap_or_else(Utc::now)
        .format("%Y%m%dT%H%M%SZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?><en-export><note><title>A &amp; B</title><created>20240102T030405Z</created><updated>20240103T030405Z</updated><tag>one</tag><note-attributes><author>Me</author><source-url>https://example.com</source-url></note-attributes><content><![CDATA[<?xml version="1.0"?><en-note><div>Hello <en-todo/>world</div></en-note>]]></content><resource><data encoding="base64">aGVsbG8=</data><mime>text/plain</mime><recognition><![CDATA[<recoIndex><item>recognized text</item></recoIndex>]]></recognition><resource-attributes><file-name>hello.txt</file-name></resource-attributes></resource></note></en-export>"#;

    #[test]
    fn parses_notes_metadata_and_resources() {
        let mut reader = EnexReader::new(Cursor::new(SAMPLE));
        let note = reader.next_note().unwrap().unwrap();
        assert_eq!(note.title, "A & B");
        assert_eq!(note.tags, ["one"]);
        assert_eq!(note.resources[0].data, b"hello");
        assert_eq!(note.attributes["author"], "Me");
        assert!(reader.next_note().unwrap().is_none());
    }

    #[test]
    fn imported_note_can_round_trip_without_edits() {
        let directory = tempfile::tempdir().unwrap();
        let library =
            Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
        let source = directory.path().join("sample.enex");
        std::fs::write(&source, SAMPLE).unwrap();
        let report = import_enex(&library, &source, Some("Test")).unwrap();
        assert_eq!(report.notes_imported, 1);
        let filter = crate::models::NoteFilter::default();
        let summaries = library.list_notes(&filter).unwrap();
        let note = library.get_note(&summaries[0].id).unwrap();
        assert_eq!(note.plain_text, "Hello world");
        assert_eq!(
            note.resources[0].recognition_text.as_deref(),
            Some("recognized text")
        );
        let output = directory.path().join("output.enex");
        export_enex(&library, std::slice::from_ref(&note), &output).unwrap();
        let mut reader = EnexReader::new(BufReader::new(File::open(output).unwrap()));
        let exported = reader.next_note().unwrap().unwrap();
        assert_eq!(exported.content, note.original_enml.unwrap());
        assert_eq!(exported.resources[0].data, b"hello");
    }
}
