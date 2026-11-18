use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
#[ignore = "requires explicitly rebuilt/bundled TestSynth and native VST3 loading"]
fn scanner_returns_stable_instrument_identity_and_rejects_a_broken_bundle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root
        .join("vendor/rust-vst3-host/test_plugins/TestSynth.vst3")
        .canonicalize()
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "clef-scan-native-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let mut previous = None;
    for index in 0..2 {
        let report = directory.join(format!("report-{index}.json"));
        let output = Command::new(env!("CARGO_BIN_EXE_clef"))
            .arg("scan-bundle")
            .arg(&fixture)
            .arg(&report)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let entries: serde_json::Value =
            serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
        assert_eq!(entries.as_array().unwrap().len(), 2);
        assert_ne!(entries[0]["id"], entries[1]["id"]);
        assert_eq!(entries[0]["format"], "VST3");
        assert!(!entries[0]["id"].as_str().unwrap().is_empty());
        if let Some(previous) = previous {
            assert_eq!(entries, previous);
        }
        previous = Some(entries);
    }
    let broken = directory.join("Broken.vst3");
    std::fs::create_dir(&broken).unwrap();
    let output = directory.join("broken.json");
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_clef"))
            .arg("scan-bundle")
            .arg(broken)
            .arg(&output)
            .status()
            .unwrap()
            .success()
    );
    assert!(!output.exists());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires rebuilt/bundled TestSynth, WindowServer and local Unix sockets"]
fn each_instrument_in_one_bundle_loads_by_id_and_restores_at_selected_sample_rate() {
    use std::{
        io::{Read, Write},
        os::unix::net::{UnixListener, UnixStream},
        process::Stdio,
        time::{Duration, Instant},
    };
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn receive(stream: &mut UnixStream) -> serde_json::Value {
        let mut size = [0; 4];
        stream.read_exact(&mut size).unwrap();
        let size = u32::from_le_bytes(size) as usize;
        assert!(size <= 1024 * 1024);
        let mut bytes = vec![0; size];
        stream.read_exact(&mut bytes).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
    fn call(stream: &mut UnixStream, request: serde_json::Value) -> serde_json::Value {
        let bytes = serde_json::to_vec(&request).unwrap();
        stream
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .unwrap();
        stream.write_all(&bytes).unwrap();
        let reply = receive(stream);
        assert!(reply["error"].is_null(), "{reply}");
        reply
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/rust-vst3-host/test_plugins/TestSynth.vst3")
        .canonicalize()
        .unwrap();
    let directory = PathBuf::from("/private/tmp").join(format!(
        "ct03-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let report = directory.join("scan.json");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_clef"))
            .args(["scan-bundle"])
            .arg(&fixture)
            .arg(&report)
            .status()
            .unwrap()
            .success()
    );
    let entries: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    for (index, plugin) in entries.as_array().unwrap().iter().enumerate() {
        let mut outputs = Vec::new();
        let mut saved = None;
        for rate in [44100, 48000] {
            let socket = directory.join(format!("s{index}-{rate}"));
            let listener = UnixListener::bind(&socket).unwrap();
            listener.set_nonblocking(true).unwrap();
            let mut child = Child(
                Command::new(env!("CARGO_BIN_EXE_clef"))
                    .arg("worker")
                    .arg(&socket)
                    .arg(&fixture)
                    .arg(plugin["id"].as_str().unwrap())
                    .arg(rate.to_string())
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap(),
            );
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut accept = || loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        break stream;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        assert!(child.0.try_wait().unwrap().is_none());
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            let mut control = accept();
            let mut audio = accept();
            let hello = receive(&mut control);
            assert_eq!(hello["name"], plugin["name"]);
            if let Some(state) = &saved {
                call(&mut audio, serde_json::json!("SuspendAudio"));
                call(&mut control, serde_json::json!({"Restore":{"state":state}}));
            }
            let snapshot = call(&mut control, serde_json::json!("Save"))["state"].clone();
            assert!(!snapshot.as_array().unwrap().is_empty());
            if let Some(state) = &saved {
                assert_eq!(&snapshot, state);
            } else {
                saved = Some(snapshot);
            }
            call(&mut audio, serde_json::json!("SuspendAudio"));
            call(&mut control, serde_json::json!({"Start":{"offline":false}}));
            let reply = call(
                &mut audio,
                serde_json::json!({"Process":{"position":0,"events":[{"offset":0,"bytes":[144,69,100]}],"frames":128}}),
            );
            let samples: Vec<f64> = reply["frames"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f[0].as_f64().unwrap())
                .collect();
            assert!(samples.iter().any(|s| s.abs() > 0.000001));
            outputs.push(samples);
            call(&mut audio, serde_json::json!("SuspendAudio"));
            call(&mut control, serde_json::json!("Stop"));
        }
        assert_ne!(
            outputs[0], outputs[1],
            "sample rate must reach the selected plugin processor"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
