use clef_engine::{graph, live, render, sequence};
use clef_plugin_host::{catalog, files, protocol, rpc, worker};
use clef_ui as app;

fn executable() -> protocol::Result<std::path::PathBuf> {
    #[cfg(test)]
    {
        Ok(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/clef"))
    }
    #[cfg(not(test))]
    {
        Ok(std::env::current_exe()?)
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> protocol::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "worker") && (4..=6).contains(&args.len()) {
        return worker::run(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            args.get(4).cloned().unwrap_or_default(),
            args.get(5)
                .map(|s| s.parse())
                .transpose()?
                .unwrap_or(protocol::RATE),
        );
    }
    if args.get(1).is_some_and(|arg| arg == "scan-bundle") && args.len() == 4 {
        return catalog::inspect(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        );
    }
    if args.len() == 1 || args.get(1).is_some_and(|s| s == "app") {
        return app::run(executable()?);
    }
    if args.get(1).is_some_and(|arg| arg == "sequence") && args.len() == 2 {
        let mut timeline = Vec::new();
        let mut block = Vec::new();
        for position in (0..sequence::LENGTH).step_by(protocol::BLOCK) {
            sequence::events(position, protocol::BLOCK, false, &mut block);
            timeline.extend(block.iter().map(|event| protocol::Midi {
                offset: position as u32 + event.offset,
                ..*event
            }));
        }
        println!(
            "{}",
            serde_json::json!({
                "start": sequence::START, "length": sequence::LENGTH, "events": timeline
            })
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|arg| arg == "devices") && args.len() == 2 {
        clef_audio_io::print_devices()?;
        return Ok(());
    }
    if args.get(1).is_some_and(|arg| arg == "plugin") && (3..=4).contains(&args.len()) {
        return console(
            std::path::Path::new(&args[2]),
            args.get(3).map(std::path::Path::new),
        );
    }
    if args.get(1).is_some_and(|arg| arg == "render") && (4..=5).contains(&args.len()) {
        let state = args
            .get(4)
            .map(|path| files::read_state(std::path::Path::new(path)))
            .transpose()?;
        let mut instrument = rpc::Instrument::load(
            &executable()?,
            std::path::Path::new(&args[2]),
            state.as_deref(),
        )?;
        let plan = if args[2] == "@fixture" {
            graph::Plan::parallel()
        } else {
            graph::Plan::direct()
        };
        let frames = render::render(&mut instrument, std::path::Path::new(&args[3]), plan)?;
        println!(
            "Rendered {frames} stereo frames at 48000 Hz, PCM24, including 10 seconds of tail."
        );
        return Ok(());
    }
    if !args
        .get(1)
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        return Err("unknown command or invalid arguments; use --help".into());
    }
    println!(
        "Clef T02 validation prototype\n  clef devices\n  clef plugin PLUGIN [STATE]\n  clef render PLUGIN WAV [STATE]\n\nInteractive commands: open, close, save PATH, restore PATH, recreate, render PATH, quit.\nLive commands: start DEVICE LEFT RIGHT, midi DEVICE LEFT RIGHT PORT, status, stop. Output channel numbers are one-based; device/port indices come from devices."
    );
    Ok(())
}

fn console(path: &std::path::Path, state_path: Option<&std::path::Path>) -> protocol::Result<()> {
    use std::io::{BufRead, Write};
    let mut saved = state_path.map(files::read_state).transpose()?;
    let mut instrument = Some(rpc::Instrument::load(
        &executable()?,
        path,
        saved.as_deref(),
    )?);
    let loaded = instrument.as_ref().unwrap();
    println!(
        "Loaded {} (latency: {} samples). Commands: open, close, save PATH, restore PATH, recreate, render PATH, quit.",
        loaded.name, loaded.latency
    );
    let mut live: Option<live::Live> = None;
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        print!("clef> ");
        std::io::stdout().flush()?;
        let Some(line) = lines.next() else {
            break;
        };
        let line = line?;
        let (command, argument) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        let file = std::path::Path::new(argument.trim());
        let result: protocol::Result<()> = (|| {
            if command == "status" {
                let live = live.as_ref().ok_or("live playback is stopped")?;
                println!("{}", live.counters.report());
                return Ok(());
            }
            if matches!(command, "stop" | "recreate" | "quit")
                && let Some(session) = live.take()
            {
                let instrument = instrument.as_mut().ok_or("worker unavailable")?;
                match session.stop(instrument) {
                    Ok(report) => println!("{report}"),
                    Err(error) => eprintln!("Live stop failed: {error}"),
                }
            }
            if command == "stop" {
                return Ok(());
            }
            if command == "recreate" {
                drop(instrument.take());
                instrument = Some(rpc::Instrument::load(
                    &executable()?,
                    path,
                    saved.as_deref(),
                )?);
                println!("Recreated worker from last saved state; unsaved edits were discarded.");
                return Ok(());
            }
            let instrument = instrument
                .as_mut()
                .ok_or("worker unavailable; use recreate")?;
            match command {
                "quit" => return Ok(()),
                "start" | "midi" => {
                    if live.is_some() {
                        return Err("stop existing playback first".into());
                    }
                    let values: Vec<usize> = argument
                        .split_whitespace()
                        .map(str::parse)
                        .collect::<std::result::Result<_, _>>()?;
                    let expected = if command == "midi" { 4 } else { 3 };
                    if values.len() != expected || values[1] == 0 || values[2] == 0 {
                        return Err("use start DEVICE LEFT RIGHT or midi DEVICE LEFT RIGHT PORT; output channels are one-based".into());
                    }
                    let plan = if path == std::path::Path::new("@fixture") {
                        graph::Plan::parallel()
                    } else {
                        graph::Plan::direct()
                    };
                    live = Some(live::Live::start(
                        instrument,
                        values[0],
                        [values[1] - 1, values[2] - 1],
                        values.get(3).copied(),
                        plan,
                    )?);
                }
                "fixture-edit" => {
                    instrument.call(protocol::Request::FixtureEdit {
                        value: argument.trim().parse()?,
                    })?;
                }
                "fault-exit" | "fault-hang" => {
                    instrument.call(protocol::Request::FixtureFault {
                        hang: command == "fault-hang",
                    })?;
                }
                "open" => {
                    instrument.editor(true)?;
                    println!("Editor opened.");
                }
                "close" => {
                    instrument.editor(false)?;
                    println!("Editor closed.");
                }
                "save" if !argument.trim().is_empty() => {
                    let state = instrument.save()?;
                    files::save_state(file, &state)?;
                    saved = Some(state);
                }
                "restore" if !argument.trim().is_empty() => {
                    let state = files::read_state(file)?;
                    instrument.restore(&state)?;
                    saved = Some(state);
                }
                "render" if !argument.trim().is_empty() => {
                    let plan = if path == std::path::Path::new("@fixture") {
                        graph::Plan::parallel()
                    } else {
                        graph::Plan::direct()
                    };
                    let frames = render::render(instrument, file, plan)?;
                    println!("Rendered {frames} frames.");
                }
                "" => {}
                _ => return Err("unknown command or missing path".into()),
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Error: {error}");
        }
        if command == "quit" {
            break;
        }
    }
    if let Some(session) = live {
        let instrument = instrument.as_mut().ok_or("worker unavailable")?;
        println!("{}", session.stop(instrument)?);
    }
    Ok(())
}
