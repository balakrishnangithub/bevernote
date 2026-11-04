use std::{env, path::PathBuf, time::Instant};

use bevernote_core::{enex::import_enex, models::NoteFilter, Library};

fn main() -> bevernote_core::Result<()> {
    let mut arguments = env::args_os().skip(1);
    let input = arguments.next().map(PathBuf::from).ok_or_else(|| {
        bevernote_core::BeverError::Invalid(
            "usage: import_enex <input.enex> <data-directory> [notebook-name]".into(),
        )
    })?;
    let data_directory = arguments.next().map(PathBuf::from).ok_or_else(|| {
        bevernote_core::BeverError::Invalid("a data directory is required".into())
    })?;
    let notebook_name = arguments
        .next()
        .map(|name| name.to_string_lossy().into_owned());
    let library = Library::open(data_directory)?;
    let started = Instant::now();
    let report = import_enex(&library, input, notebook_name.as_deref())?;
    let notes = library.list_notes(&NoteFilter {
        notebook_id: Some(report.notebook_id.clone()),
        ..NoteFilter::default()
    })?;
    println!(
        "Imported {} notes, {} attachments ({} bytes) in {:.2?}; {} searchable notes.",
        report.notes_imported,
        report.resources_imported,
        report.bytes_imported,
        started.elapsed(),
        notes.len()
    );
    for note in notes {
        println!("- {} ({})", note.title, note.id);
    }
    Ok(())
}
