use clef_plugin_host::{
    catalog::Catalog,
    protocol::{BLOCK, Frame, RATE},
    rpc::Instrument,
};
use std::{
    path::Path,
    process::Command,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires explicitly bundled T12 insert fixtures and macOS WindowServer"]
fn stereo_effect_workers_scan_process_restart_and_restore() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("validation-output/fixtures");
    let executable = Path::new(env!("CARGO_BIN_EXE_clef"));
    for name in ["ClefInsertVST3.vst3", "ClefInsertCLAP.clap"] {
        let path = fixture_dir.join(name);
        let report = std::env::temp_dir().join(format!("clef-{}-{name}.json", std::process::id()));
        assert!(
            Command::new(executable)
                .arg("scan-bundle-categorized")
                .arg(&path)
                .arg(&report)
                .status()
                .unwrap()
                .success(),
            "{name} scan failed"
        );
        let catalog: Catalog = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        std::fs::remove_file(report).unwrap();
        assert!(
            catalog.plugins.is_empty(),
            "{name} appeared as an instrument"
        );
        assert_eq!(catalog.effects.len(), 1, "{name} effect count");
        let plugin = &catalog.effects[0];
        let load = |state: Option<&[u8]>| {
            Instrument::load_effect(
                executable,
                &path,
                &plugin.id,
                RATE,
                state,
                &AtomicBool::new(false),
            )
            .unwrap()
        };
        let mut worker = load(None);
        worker.start(false).unwrap();
        let control = worker.control_connection();
        let mut audio = worker.take_audio().unwrap();
        let deadline = Duration::from_secs(2);
        let mut source = [[0.0; 2]; BLOCK];
        source[0] = [0.5, -0.25];
        let reply = audio
            .process_effect(0, &source, deadline)
            .unwrap()
            .expect("block answered on time");
        assert_eq!(reply.latency, 0, "{name}");
        assert_eq!(reply.frames[0], [0.25, -0.125], "{name}");

        let mut position = BLOCK as u64;
        let wait = Instant::now() + Duration::from_secs(8);
        let zeros: [Frame; BLOCK] = [[0.0; 2]; BLOCK];
        loop {
            let reply = audio
                .process_effect(position, &zeros, deadline)
                .unwrap()
                .expect("block answered on time");
            position += BLOCK as u64;
            if reply.restart {
                assert_eq!(audio.reconfigure(&control).unwrap(), 257, "{name}");
                break;
            }
            assert!(
                Instant::now() < wait,
                "{name} latency restart was not delivered"
            );
        }
        audio.suspend().unwrap();
        worker.return_audio(audio);
        worker.stop().unwrap();
        let state = worker.save().unwrap();
        drop(worker);

        let mut reopened = load(Some(&state));
        reopened.start(false).unwrap();
        let mut audio = reopened.take_audio().unwrap();
        assert_eq!(audio.latency, 257, "{name} restored latency");
        let mut frames = Vec::new();
        for input in [&source, &zeros, &zeros] {
            frames.extend(
                audio
                    .process_effect(frames.len() as u64, input, deadline)
                    .unwrap()
                    .expect("block answered on time")
                    .frames,
            );
        }
        assert_eq!(frames[257], [0.25, -0.125], "{name} restored impulse");
        assert_eq!(
            frames.iter().filter(|&&frame| frame != [0.0; 2]).count(),
            1,
            "{name}"
        );
    }
}
