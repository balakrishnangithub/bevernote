use std::{env, path::PathBuf};

use bevernote_core::{
    enex::{export_enex, import_enex},
    models::{Note, NoteFilter},
    BeverError, Library,
};

fn main() -> bevernote_core::Result<()> {
    let input = env::args_os().nth(1).map(PathBuf::from).ok_or_else(|| {
        BeverError::Invalid("usage: verify_enex <input.enex> <empty-work-directory>".into())
    })?;
    let work = env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| BeverError::Invalid("an empty work directory is required".into()))?;
    if work.exists() {
        return Err(BeverError::Invalid(
            "work directory must not already exist".into(),
        ));
    }
    let source = Library::open(work.join("source"))?;
    let report = import_enex(&source, input, Some("Verification"))?;
    let mut notes = all_notes(&source)?;
    assert!(!notes.is_empty(), "the sample should contain notes");
    let title_query = format!("\"{}\"", notes[0].title.replace('"', ""));
    assert!(
        !source
            .list_notes(&NoteFilter {
                query: title_query,
                ..NoteFilter::default()
            })?
            .is_empty(),
        "imported note title must be searchable"
    );

    let exported = work.join("roundtrip.enex");
    notes.sort_by(|left, right| left.title.cmp(&right.title));
    export_enex(&source, &notes, &exported)?;
    let restored = Library::open(work.join("restored"))?;
    import_enex(&restored, &exported, Some("Restored"))?;
    let mut restored_notes = all_notes(&restored)?;
    restored_notes.sort_by(|left, right| left.title.cmp(&right.title));
    assert_eq!(notes.len(), restored_notes.len());
    for (before, after) in notes.iter().zip(&restored_notes) {
        assert_eq!(
            fingerprint(before),
            fingerprint(after),
            "round-trip changed {}",
            before.title
        );
    }
    println!(
        "Verified {} notes and {} resources: original ENML, metadata, attachment hashes, and offline title search survived export/reimport.",
        report.notes_imported, report.resources_imported
    );
    Ok(())
}

fn all_notes(library: &Library) -> bevernote_core::Result<Vec<Note>> {
    library
        .list_notes(&NoteFilter::default())?
        .iter()
        .map(|summary| library.get_note(&summary.id))
        .collect()
}

fn fingerprint(note: &Note) -> String {
    serde_json::to_string(&(
        &note.title,
        &note.original_enml,
        &note.attributes,
        &note.tags,
        note.resources
            .iter()
            .map(|resource| {
                (
                    &resource.sha256,
                    &resource.md5,
                    &resource.mime,
                    &resource.file_name,
                    resource.size,
                    resource.width,
                    resource.height,
                    resource.duration,
                    &resource.recognition_xml,
                    &resource.attributes,
                    &resource.alternate_sha256,
                    resource.position,
                )
            })
            .collect::<Vec<_>>(),
    ))
    .expect("note fingerprint is serializable")
}
