use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
#[ignore = "requires macOS WindowServer and permission for local Unix sockets"]
fn isolated_eframe_fixture_renders_exact_aligned_pcm24_without_overwriting() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path: PathBuf = std::env::temp_dir().join(format!("clef-native-{nonce}.wav"));
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_clef"))
            .arg("render")
            .arg("@fixture")
            .arg(&path)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&path).unwrap();
    let mut reader = hound::WavReader::open(&path).unwrap();
    assert_eq!(reader.duration(), 2_323_200);
    assert_eq!(
        reader.spec(),
        hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        }
    );
    let samples: Vec<_> = reader
        .samples::<i32>()
        .take(128)
        .map(Result::unwrap)
        .collect();
    assert_eq!(&samples[74..76], &[4_194_304, 4_194_304]);
    assert!(samples[..74].iter().all(|&sample| sample == 0));
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires rebuilt/bundled TestSynth, macOS WindowServer and local Unix sockets"]
fn sdk_fixture_preserves_state_across_a_fresh_worker_and_renders_nonzero_audio() {
    use std::{io::Write, process::Stdio};
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/rust-vst3-host/test_plugins/TestSynth.vst3");
    assert!(
        fixture.exists(),
        "build and bundle TestSynth explicitly first"
    );
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("clef-sdk-state-{nonce}"));
    std::fs::create_dir(&directory).unwrap();
    let state = directory.join("snapshot.state");
    let wav = directory.join("restored.wav");
    let mut child = Command::new(env!("CARGO_BIN_EXE_clef"))
        .arg("plugin")
        .arg(&fixture)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.take().unwrap(),
        "save {}\nrecreate\nrender {}\nquit",
        state.display(),
        wav.display()
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("Error:"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot = std::fs::read(&state).unwrap();
    assert!(snapshot.starts_with(b"VST3HOST_STATE\0\0"));
    let mut reader = hound::WavReader::open(&wav).unwrap();
    assert_eq!(reader.duration(), 2_323_200);
    assert!(reader.samples::<i32>().any(|s| s.unwrap().abs() > 1000));
    std::fs::remove_dir_all(directory).unwrap();
}
