use clef_plugin_host::{
    protocol::{BLOCK, RATE},
    rpc::Instrument,
};
use std::path::Path;

#[test]
#[ignore = "requires the built Clef worker and macOS WindowServer"]
fn an_unavailable_editor_reply_does_not_terminate_the_worker() {
    let executable = Path::new(env!("CARGO_BIN_EXE_clef"));
    let mut worker =
        Instrument::load_selected(executable, Path::new("@fixture"), "", RATE, None).unwrap();
    worker.start(false).unwrap();
    let mut audio = worker.take_audio().unwrap();
    assert!(worker.editor(true).is_err());
    assert!(!worker.exited());
    assert!(audio.process(0, &[], BLOCK).is_ok());
    audio.suspend().unwrap();
    worker.return_audio(audio);
    worker.stop().unwrap();
}

#[test]
#[ignore = "requires built Clef, bundled TestSynth and macOS WindowServer"]
fn test_synth_editor_opens_and_its_worker_keeps_processing() {
    let executable = Path::new(env!("CARGO_BIN_EXE_clef"));
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/rust-vst3-host/test_plugins/TestSynth.vst3")
        .canonicalize()
        .unwrap();
    let mut worker = Instrument::load(executable, &fixture, None).unwrap();
    worker.start(false).unwrap();
    let mut audio = worker.take_audio().unwrap();
    worker.editor(true).unwrap();
    assert!(!worker.exited());
    assert!(audio.process(0, &[], BLOCK).is_ok());
    worker.editor(false).unwrap();
    worker.editor(true).unwrap();
    assert!(!worker.exited());
    assert!(audio.process(0, &[], BLOCK).is_ok());
    worker.editor(false).unwrap();
    audio.suspend().unwrap();
    worker.return_audio(audio);
    worker.stop().unwrap();
}
