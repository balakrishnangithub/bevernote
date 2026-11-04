use std::{fs, io::Cursor};

use bevernote_core::{
    enex::{export_enex, import_enex, EnexReader},
    enml::enml_to_html,
    models::{NoteFilter, SaveNote},
    Library,
};
use serde_json::json;

const DUPLICATE_GUID_SAMPLE: &str = r#"<?xml version="1.0"?><en-export><note><title>First</title><guid>01234567-89ab-cdef-0123-456789abcdef</guid><created>20240102T030405Z</created><content><![CDATA[<en-note><div>One</div></en-note>]]></content><note-attributes><reminder-order>7</reminder-order></note-attributes></note><note><title>Second</title><guid>01234567-89ab-cdef-0123-456789abcdef</guid><created>20240102T030405Z</created><content><![CDATA[<en-note><div>Two</div></en-note>]]></content></note></en-export>"#;

#[test]
fn modern_evernote_checklists_become_editable_task_items() {
    let html = enml_to_html(
        r#"<en-note><ul><li style="--en-todo:true;--en-checked:false;">Later</li><li style="--en-todo:true;--en-checked:true;">Done</li></ul></en-note>"#,
    );
    assert!(html.contains("data-type=\"taskList\""));
    assert!(html.contains("data-checked=\"false\""));
    assert!(html.contains("data-checked=\"true\""));
    assert!(html.contains("Later"));
}

#[test]
fn duplicate_guids_do_not_prevent_reimport_and_attributes_keep_values() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    let source = directory.path().join("duplicates.enex");
    fs::write(&source, DUPLICATE_GUID_SAMPLE).unwrap();
    let first_import = import_enex(&library, &source, Some("One")).unwrap();
    assert_eq!(first_import.notes_imported, 2);
    assert_eq!(first_import.notes_skipped, 0);
    let repeated = import_enex(&library, &source, Some("Two")).unwrap();
    assert_eq!(repeated.notes_imported, 0);
    assert_eq!(repeated.notes_skipped, 2);
    assert_eq!(library.list_notes(&NoteFilter::default()).unwrap().len(), 2);

    let first = library
        .list_notes(&NoteFilter::default())
        .unwrap()
        .remove(0);
    let mut note = library.get_note(&first.id).unwrap();
    note.content_html = "<p>My local edit must survive reimport.</p>".into();
    note.attributes
        .as_object_mut()
        .unwrap()
        .insert("reminder-done".into(), json!(true));
    note.attributes
        .as_object_mut()
        .unwrap()
        .insert("latitude".into(), json!(12.5));
    let saved = library
        .save_note(&SaveNote {
            id: note.id.clone(),
            notebook_id: note.notebook_id.clone(),
            title: note.title.clone(),
            content_html: note.content_html.clone(),
            attributes: note.attributes.clone(),
            tags: note.tags.clone(),
        })
        .unwrap();
    let exported = directory.path().join("attributes.enex");
    export_enex(&library, &[saved], &exported).unwrap();
    let text = fs::read_to_string(exported).unwrap();
    assert!(text.contains("<reminder-order>7</reminder-order>"));
    assert!(text.contains("<reminder-done>true</reminder-done>"));
    assert!(text.contains("<latitude>12.5</latitude>"));

    let edited_reimport = import_enex(&library, &source, Some("Three")).unwrap();
    assert_eq!(edited_reimport.notes_skipped, 2);
    assert_eq!(
        library.get_note(&first.id).unwrap().content_html,
        "<p>My local edit must survive reimport.</p>"
    );
}

#[test]
fn notes_without_guids_also_deduplicate() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    let source = directory.path().join("without-guid.enex");
    fs::write(
        &source,
        r#"<?xml version="1.0"?><en-export><note><title>No GUID</title><created>20240102T030405Z</created><content><![CDATA[<en-note><div>Original</div></en-note>]]></content></note></en-export>"#,
    )
    .unwrap();
    assert_eq!(
        import_enex(&library, &source, None).unwrap().notes_imported,
        1
    );
    let report = import_enex(&library, &source, None).unwrap();
    assert_eq!(report.notes_imported, 0);
    assert_eq!(report.notes_skipped, 1);
    assert_eq!(library.list_notes(&NoteFilter::default()).unwrap().len(), 1);
}

#[test]
#[ignore = "OCR inference is intentionally tested in release mode"]
fn local_ocr_extracts_image_text_into_offline_search() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    let source = directory.path().join("ocr.enex");
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        include_bytes!("fixtures/ocr-invoice.png"),
    );
    fs::write(
        &source,
        format!(r#"<?xml version="1.0"?><en-export><note><title>Scanned receipt</title><created>20240102T030405Z</created><content><![CDATA[<en-note><div>Scan</div></en-note>]]></content><resource><data encoding="base64">{encoded}</data><mime>image/png</mime></resource></note></en-export>"#),
    )
    .unwrap();
    let report = import_enex(&library, &source, None).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let notes = library.all_notes().unwrap();
    let ocr = notes[0].resources[0]
        .recognition_text
        .as_deref()
        .unwrap_or("");
    assert!(ocr.to_lowercase().contains("invoice"), "OCR result: {ocr}");
    let results = library
        .list_notes(&NoteFilter {
            query: "invoice".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn attachments_can_be_retrieved_by_id_and_content_hash() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    let note = library.create_note(None).unwrap();
    let source = directory.path().join("report.pdf");
    fs::write(&source, b"%PDF-1.4\nattachment test\n").unwrap();
    let attached = library.attach_file(&note.id, &source).unwrap();

    let by_id = library.resource(&attached.id).unwrap();
    assert_eq!(by_id.file_name.as_deref(), Some("report.pdf"));
    assert_eq!(by_id.mime, "application/pdf");
    assert_eq!(by_id.size, 25);
    let by_md5 = library.resource_location(&attached.md5).unwrap();
    let by_sha256 = library.resource_location(&attached.sha256).unwrap();
    assert_eq!(by_md5.id, attached.id);
    assert_eq!(by_sha256.id, attached.id);
    assert_eq!(
        library.read_blob(&by_id.sha256).unwrap(),
        b"%PDF-1.4\nattachment test\n"
    );
}

#[test]
fn reader_handles_cdata_and_multiple_notes_without_losing_text() {
    let mut reader = EnexReader::new(Cursor::new(DUPLICATE_GUID_SAMPLE));
    assert_eq!(reader.next_note().unwrap().unwrap().title, "First");
    assert_eq!(reader.next_note().unwrap().unwrap().title, "Second");
    assert!(reader.next_note().unwrap().is_none());
}

#[test]
fn import_progress_exposes_notebook_before_notes_and_finishes_completely() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    library.set_setting("ocr_enabled", "false").unwrap();
    let source = directory.path().join("progress.enex");
    fs::write(&source, DUPLICATE_GUID_SAMPLE).unwrap();
    let mut events = Vec::new();
    let report = bevernote_core::enex::import_enex_with_progress(
        &library,
        &source,
        Some("Progress"),
        |progress| {
            events.push((
                progress.phase,
                progress.note_index,
                progress.bytes_read,
                progress.total_bytes,
                progress.notebook_id,
            ));
        },
    )
    .unwrap();

    assert_eq!(events.first().unwrap().0, "preparing");
    let notebook_ready = events
        .iter()
        .position(|event| event.0 == "notebookReady")
        .unwrap();
    let first_note = events
        .iter()
        .position(|event| event.0 == "noteImported")
        .unwrap();
    assert!(notebook_ready < first_note);
    assert!(events
        .iter()
        .any(|event| event.0 == "finalizing" && event.1 == 2));
    let complete = events.last().unwrap();
    assert_eq!(complete.0, "complete");
    assert_eq!(complete.1, 2);
    assert_eq!(complete.2, source.metadata().unwrap().len());
    assert_eq!(complete.2, complete.3);
    assert!(events
        .iter()
        .filter(|event| event.0 == "notebookReady" || event.0 == "noteImported")
        .all(|event| event.4 == report.notebook_id));
}

#[test]
fn copied_notes_have_new_identity_but_reuse_attachment_blobs() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    library.set_setting("ocr_enabled", "false").unwrap();
    let source = directory.path().join("copy-source.enex");
    fs::write(
        &source,
        r#"<?xml version="1.0"?><en-export><note><title>Source note</title><guid>11234567-89ab-cdef-0123-456789abcdef</guid><created>20240102T030405Z</created><updated>20240102T030405Z</updated><content><![CDATA[<en-note><div>Copied body</div><en-media hash="2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae" type="text/plain"/></en-note>]]></content><tag>original</tag><resource><data encoding="base64">YXR0YWNobWVudCBib2R5</data><mime>text/plain</mime><resource-attributes><file-name>source.txt</file-name></resource-attributes></resource></note></en-export>"#,
    )
    .unwrap();
    import_enex(&library, &source, Some("Imports")).unwrap();
    let original = library.all_notes().unwrap().remove(0);
    let destination = library.ensure_notebook("Copies", None).unwrap();

    assert_eq!(
        library
            .copy_notes(std::slice::from_ref(&original.id), &destination)
            .unwrap(),
        1
    );
    let copied = library
        .all_notes()
        .unwrap()
        .into_iter()
        .find(|note| note.id != original.id)
        .unwrap();
    assert_eq!(copied.title, "Source note (copy)");
    assert_eq!(copied.notebook_id, destination);
    assert!(copied.evernote_guid.is_none());
    assert!(original.evernote_guid.is_some());
    assert!(copied.created_at > original.created_at);
    assert_eq!(copied.tags, original.tags);
    assert_eq!(copied.resources.len(), 1);
    assert_ne!(copied.resources[0].id, original.resources[0].id);
    assert_eq!(copied.resources[0].note_id, copied.id);
    assert_eq!(copied.resources[0].sha256, original.resources[0].sha256);
    assert_eq!(
        library.read_blob(&copied.resources[0].sha256).unwrap(),
        b"attachment body"
    );

    let repeated = import_enex(&library, &source, Some("Imports")).unwrap();
    assert_eq!(repeated.notes_imported, 0);
    assert_eq!(repeated.notes_skipped, 1);
    assert_eq!(library.all_notes().unwrap().len(), 2);
}

#[test]
fn bulk_move_and_additive_tagging_are_transactional_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    let first = library.create_note(None).unwrap();
    let second = library.create_note(None).unwrap();
    let destination = library.ensure_notebook("Destination", None).unwrap();
    assert_eq!(
        library
            .copy_notes(std::slice::from_ref(&second.id), &destination)
            .unwrap(),
        1
    );
    assert!(library
        .all_notes()
        .unwrap()
        .iter()
        .any(|note| note.title == "Untitled (copy)"));

    let ids = vec![first.id.clone(), second.id.clone()];
    assert_eq!(library.move_notes(&ids, &destination).unwrap(), 2);
    assert!(library
        .all_notes()
        .unwrap()
        .iter()
        .filter(|note| ids.contains(&note.id))
        .all(|note| note.notebook_id == destination));
    let tags = vec!["Review".to_owned(), "follow-up".to_owned()];
    assert_eq!(library.add_tags_to_notes(&ids, &tags).unwrap(), 2);
    assert_eq!(library.add_tags_to_notes(&ids, &tags).unwrap(), 0);
    for id in &ids {
        let note = library.get_note(id).unwrap();
        assert!(note.tags.contains(&"Review".to_owned()));
        assert!(note.tags.contains(&"follow-up".to_owned()));
    }
    assert!(library.move_notes(&ids, "missing-notebook").is_err());
    assert!(library
        .all_notes()
        .unwrap()
        .iter()
        .filter(|note| ids.contains(&note.id))
        .all(|note| note.notebook_id == destination));
}

#[test]
fn note_timestamps_can_be_corrected_without_accepting_invalid_ranges() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    let note = library.create_note(None).unwrap();
    let created_at = 1_704_164_645_000;
    let updated_at = created_at + 60_000;
    let updated = library
        .update_note_timestamps(&note.id, created_at, updated_at)
        .unwrap();
    assert_eq!(updated.created_at, created_at);
    assert_eq!(updated.updated_at, updated_at);
    assert_eq!(updated.revision, note.revision + 1);
    assert!(library
        .update_note_timestamps(&note.id, created_at, created_at - 1)
        .is_err());
    assert!(library
        .update_note_timestamps(&note.id, -1, updated_at)
        .is_err());
    let unchanged = library.get_note(&note.id).unwrap();
    assert_eq!(unchanged.created_at, created_at);
    assert_eq!(unchanged.updated_at, updated_at);
}

#[test]
fn repeated_tags_in_one_enex_note_do_not_abort_import() {
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    library.set_setting("ocr_enabled", "false").unwrap();
    let source = directory.path().join("repeated-tags.enex");
    fs::write(
        &source,
        r#"<?xml version="1.0"?><en-export><note><title>Repeated tags</title><created>20240102T030405Z</created><content><![CDATA[<en-note><div>Body</div></en-note>]]></content><tag>Review</tag><tag>Review</tag><tag>review</tag></note></en-export>"#,
    )
    .unwrap();
    let report = import_enex(&library, &source, None).unwrap();
    assert_eq!(report.notes_imported, 1);
    let notes = library.all_notes().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].tags.len(), 1);
    assert_eq!(notes[0].tags[0].to_lowercase(), "review");
    let repeated = import_enex(&library, &source, None).unwrap();
    assert_eq!(repeated.notes_imported, 0);
    assert_eq!(repeated.notes_skipped, 1);
}

#[test]
#[ignore = "set BEVERNOTE_LARGE_ENEX to a local private export to run this bounded check"]
fn private_large_enex_streams_with_ordered_progress() {
    let Ok(source) = std::env::var("BEVERNOTE_LARGE_ENEX") else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let library =
        Library::open_at(directory.path().join("db"), directory.path().join("blobs")).unwrap();
    // Keep this bounded check focused on streaming/progress; OCR is tested separately.
    library.set_setting("ocr_enabled", "false").unwrap();
    let started = std::time::Instant::now();
    let mut notebook_ready = false;
    let mut complete = false;
    let mut note_index = 0;
    let mut progress_events = 0_u64;
    let report = bevernote_core::enex::import_enex_with_progress(
        &library,
        source,
        Some("Large import verification"),
        |progress| {
            progress_events += 1;
            match progress.phase.as_str() {
                "notebookReady" => notebook_ready = true,
                "noteImported" => {
                    assert!(
                        notebook_ready,
                        "notebook must be visible before note events"
                    );
                    note_index = progress.note_index;
                }
                "complete" => complete = true,
                _ => {}
            }
        },
    )
    .unwrap();
    assert!(notebook_ready);
    assert!(complete);
    assert_eq!(report.notes_imported + report.notes_skipped, note_index);
    eprintln!(
        "Private large ENEX: {note_index} notes, {} attachments, {progress_events} progress events, {} warnings, {:.1?}; OCR disabled",
        report.resources_imported,
        report.warnings.len(),
        started.elapsed(),
    );
}
