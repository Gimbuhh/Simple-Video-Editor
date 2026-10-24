//! Migration inspector, not the desktop editor. Never opens referenced media
//! and has no write, save, recovery, playback, or export path.
use std::{env, fs::File, process::ExitCode};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("Usage: sve-inspect <project.sveproject>")?;
    if args.next().is_some() {
        return Err("Usage: sve-inspect <project.sveproject>".into());
    }
    let project = project_store::read_project(File::open(path)?)?;
    let clips: Vec<_> = project
        .timeline
        .clips()
        .iter()
        .map(|c| {
            serde_json::json!({
                "clipId":c.id.0, "sourceId":c.source.0, "sourceIn":c.source_in,
                "sourceOut":c.source_out, "timelineStart":c.timeline_start
            })
        })
        .collect();
    // Paths are deliberately omitted: CLI/CI output need not disclose private
    // recording filenames. IDs are stable within this imported document only.
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion":1, "sourceCount":project.sources.len(),
            "clipCount":clips.len(), "durationSeconds":project.timeline.duration(), "clips":clips
        }))?
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
