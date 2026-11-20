use crate::protocol::{Frame, MAX_STATE, Midi, Reply, Request, Result, validate_block};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, TryLockError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_PACKET: usize = MAX_STATE * 4 + 8192;
const MAX_AUDIO_PACKET: usize = 128 * 1024;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Requests that legitimately take long: worker startup, editor open, and state restore.
const LONG_CONTROL: Duration = Duration::from_secs(60);
/// State capture can serialize large plugin state but must not hold the session for a minute.
const SAVE_CONTROL: Duration = Duration::from_secs(10);
/// Every other control request (start, stop, editor close, quit, fixture controls).
const SHORT_CONTROL: Duration = Duration::from_secs(5);

/// Return the control deadline for `request`; a control timeout terminates the worker.
pub fn control_timeout(request: &Request) -> Duration {
    match request {
        Request::Restore { .. } | Request::Editor { open: true } => LONG_CONTROL,
        Request::Save => SAVE_CONTROL,
        _ => SHORT_CONTROL,
    }
}

/// Process blocks a late effect may have queued before further blocks are skipped as dry.
const MAX_IN_FLIGHT: usize = 2;

fn terminate(child: &Mutex<Child>) {
    let mut child = child.lock().unwrap_or_else(|error| error.into_inner());
    let _ = child.kill();
    let _ = child.wait();
}

pub fn send<T: Serialize>(stream: &mut impl Write, value: &T) -> Result<()> {
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_PACKET {
        return Err("IPC packet exceeds limit".into());
    }
    stream.write_all(&(data.len() as u32).to_le_bytes())?;
    stream.write_all(&data)?;
    Ok(())
}

/// Borrowed form of the audio-connection requests, serialized like [`Request`] without
/// copying the block's events and input.
#[derive(Serialize)]
enum AudioRequest<'a> {
    Process {
        position: u64,
        events: &'a [Midi],
        frames: usize,
        #[serde(skip_serializing_if = "<[Frame]>::is_empty")]
        input: &'a [Frame],
        sequence: u64,
    },
    SuspendAudio,
}

pub fn receive<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    receive_limited(stream, MAX_PACKET)
}

pub fn receive_audio<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T> {
    receive_limited(stream, MAX_AUDIO_PACKET)
}

fn receive_limited<T: DeserializeOwned>(stream: &mut impl Read, limit: usize) -> Result<T> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_le_bytes(header) as usize;
    if length == 0 || length > limit {
        return Err("invalid IPC packet length".into());
    }
    let mut data = vec![0; length];
    stream.read_exact(&mut data)?;
    Ok(serde_json::from_slice(&data)?)
}

/// Recomputes the remaining timeout on every read/write, including short transfers.
struct DeadlineStream<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}

impl DeadlineStream<'_> {
    fn remaining(&self) -> std::io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "worker exchange deadline exceeded",
                )
            })
    }
}
impl<'a> DeadlineStream<'a> {
    fn new(stream: &'a mut UnixStream, deadline: Instant) -> Result<Self> {
        // Darwin can reject SO_RCVTIMEO after a peer sends its failure reply and closes.
        stream.set_nonblocking(true)?;
        Ok(Self { stream, deadline })
    }
    fn wait(&self, writable: bool) -> std::io::Result<()> {
        let mut descriptor = libc::pollfd {
            fd: self.stream.as_raw_fd(),
            events: if writable {
                libc::POLLOUT
            } else {
                libc::POLLIN
            },
            revents: 0,
        };
        let timeout = self
            .remaining()?
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        // poll wakes on socket readiness; a periodic sleep can miss the audio budget.
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        self.remaining()?;
        Ok(())
    }
}
impl Read for DeadlineStream<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        loop {
            self.remaining()?;
            match self.stream.read(bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.wait(false)?,
                result => return result,
            }
        }
    }
}
impl Write for DeadlineStream<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        loop {
            self.remaining()?;
            match self.stream.write(bytes) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.wait(true)?,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Waits for socket readiness with sub-millisecond timeouts, so an audio exchange never
/// overshoots its block deadline by a rounded-up millisecond.
struct Readiness {
    #[cfg(target_vendor = "apple")]
    queue: std::os::fd::OwnedFd,
}

impl Readiness {
    fn new() -> std::io::Result<Self> {
        #[cfg(target_vendor = "apple")]
        {
            use std::os::fd::FromRawFd;
            let queue = unsafe { libc::kqueue() };
            if queue < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // kqueue descriptors are not inherited by spawned children.
            Ok(Self {
                queue: unsafe { std::os::fd::OwnedFd::from_raw_fd(queue) },
            })
        }
        #[cfg(not(target_vendor = "apple"))]
        Ok(Self {})
    }

    /// Return after readiness, a signal, or `timeout`; callers re-check their state.
    fn wait(&self, fd: RawFd, writable: bool, timeout: Duration) -> std::io::Result<()> {
        let timespec = libc::timespec {
            tv_sec: timeout.as_secs().min(i32::MAX as u64) as libc::time_t,
            tv_nsec: timeout.subsec_nanos() as _,
        };
        #[cfg(target_vendor = "apple")]
        let result = {
            use std::os::fd::AsRawFd;
            let event = |filter| libc::kevent {
                ident: fd as libc::uintptr_t,
                filter,
                flags: libc::EV_ADD | libc::EV_ONESHOT,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            let changes = [event(libc::EVFILT_READ), event(libc::EVFILT_WRITE)];
            let mut events = [event(0), event(0)];
            unsafe {
                libc::kevent(
                    self.queue.as_raw_fd(),
                    changes.as_ptr(),
                    if writable { 2 } else { 1 },
                    events.as_mut_ptr(),
                    2,
                    &timespec,
                )
            }
        };
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let result = {
            let mut descriptor = libc::pollfd {
                fd,
                events: libc::POLLIN | if writable { libc::POLLOUT } else { 0 },
                revents: 0,
            };
            unsafe { libc::ppoll(&mut descriptor, 1, &timespec, std::ptr::null()) }
        };
        #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
        let result = {
            let _ = timespec;
            let mut descriptor = libc::pollfd {
                fd,
                events: libc::POLLIN | if writable { libc::POLLOUT } else { 0 },
                revents: 0,
            };
            let millis = timeout.as_micros().div_ceil(1000).min(i32::MAX as u128) as i32;
            unsafe { libc::poll(&mut descriptor, 1, millis) }
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ControlConnection {
    stream: Arc<Mutex<UnixStream>>,
    // A timed-out exchange kills the worker; the shared child prevents PID-reuse races.
    child: Arc<Mutex<Child>>,
}

impl ControlConnection {
    /// Capture plugin state; the plugin must be stopped.
    pub fn save(&self) -> Result<Vec<u8>> {
        let state = self
            .call_until(&Request::Save, Instant::now() + SAVE_CONTROL)?
            .state;
        if state.len() > MAX_STATE {
            return Err("plugin state exceeds limit".into());
        }
        Ok(state)
    }

    fn call_until(&self, request: &Request, deadline: Instant) -> Result<Reply> {
        let reply = self.call_reply(request, deadline)?;
        if let Some(error) = &reply.error {
            return Err(error.clone().into());
        }
        if reply.latency > crate::protocol::RATE * 10 {
            return Err("worker latency exceeds prototype capacity".into());
        }
        Ok(reply)
    }

    fn call_reply(&self, request: &Request, deadline: Instant) -> Result<Reply> {
        let mut stream = loop {
            match self.stream.try_lock() {
                Ok(stream) => break stream,
                Err(TryLockError::Poisoned(_)) => return Err("control connection poisoned".into()),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_micros(100))
                }
                Err(_) => return Err("control connection deadline exceeded".into()),
            }
        };
        let result = DeadlineStream::new(&mut stream, deadline).and_then(|mut stream| {
            send(&mut stream, request)?;
            receive::<Reply>(&mut stream)
        });
        if result.is_err() {
            // A timed-out or broken exchange can leave a partial request or an unread reply.
            // Terminate the worker so no later call can read that reply as its own answer.
            let _ = stream.shutdown(std::net::Shutdown::Both);
            drop(stream);
            terminate(&self.child);
        }
        result
    }
}

pub struct Instrument {
    processing_mode: Option<bool>,
    child: Arc<Mutex<Child>>,
    control: ControlConnection,
    audio: Option<AudioConnection>,
    // A native close can arrive after the next editor has already opened.
    editor_events: UnixStream,
    editor_generation: u8,
    pub name: String,
    pub latency: u32,
}

struct SocketPath(PathBuf);
impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

impl Instrument {
    pub fn load(executable: &Path, path: &Path, state: Option<&[u8]>) -> Result<Self> {
        Self::load_selected(executable, path, "", crate::protocol::RATE, state)
    }

    pub fn load_selected(
        executable: &Path,
        path: &Path,
        selected: &str,
        rate: u32,
        state: Option<&[u8]>,
    ) -> Result<Self> {
        Self::load_selected_cancellable(
            executable,
            path,
            selected,
            rate,
            state,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }

    pub fn load_selected_cancellable(
        executable: &Path,
        path: &Path,
        selected: &str,
        rate: u32,
        state: Option<&[u8]>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        Self::load_worker(executable, path, selected, rate, state, cancel, false)
    }

    pub fn load_effect(
        executable: &Path,
        path: &Path,
        selected: &str,
        rate: u32,
        state: Option<&[u8]>,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        Self::load_worker(executable, path, selected, rate, state, cancel, true)
    }

    fn load_worker(
        executable: &Path,
        path: &Path,
        selected: &str,
        rate: u32,
        state: Option<&[u8]>,
        cancel: &std::sync::atomic::AtomicBool,
        effect: bool,
    ) -> Result<Self> {
        if !(8000..=384000).contains(&rate) {
            return Err("unsupported sample rate".into());
        }
        let path = if path == Path::new("@fixture") {
            path.to_owned()
        } else {
            path.canonicalize()?
        };
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        use std::os::unix::fs::DirBuilderExt;
        // macOS sockaddr_un cannot hold the long per-user TMPDIR plus this private name.
        let directory =
            Path::new("/private/tmp").join(format!("clef-{}-{nonce}-{serial}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| {
                format!(
                    "create worker socket directory {}: {e}",
                    directory.display()
                )
            })?;
        let socket = SocketPath(directory.join("rpc"));
        let listener = UnixListener::bind(&socket.0)
            .map_err(|error| format!("bind worker socket {}: {error}", socket.0.display()))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("configure worker listener: {error}"))?;
        let mut child = Command::new(executable)
            .arg(if effect { "effect-worker" } else { "worker" })
            .arg(&socket.0)
            .arg(path)
            .arg(selected)
            .arg(rate.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let deadline = Instant::now() + LONG_CONTROL;
        let mut accept = || -> Result<UnixStream> {
            loop {
                if cancel.load(std::sync::atomic::Ordering::Acquire) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("load cancelled".into());
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false)?;
                        return Ok(stream);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if let Some(status) = child.try_wait()? {
                            return Err(format!("plugin worker exited: {status}").into());
                        }
                        if Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err("plugin worker startup timed out".into());
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(e.into());
                    }
                }
            }
        };
        let streams = (|| -> Result<_> {
            let stream = accept()?;
            let audio = AudioConnection::new(accept()?)?;
            let editor_events = accept()?;
            editor_events.set_nonblocking(true)?;
            Ok((stream, audio, editor_events))
        })();
        let (mut stream, audio, editor_events) = match streams {
            Ok(streams) => streams,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let hello: Reply = match DeadlineStream::new(&mut stream, deadline)
            .and_then(|mut stream| receive(&mut stream))
        {
            Ok(reply) => reply,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("receive worker startup reply: {error}").into());
            }
        };
        let child = Arc::new(Mutex::new(child));
        let mut instrument = Self {
            processing_mode: None,
            child: Arc::clone(&child),
            control: ControlConnection {
                stream: Arc::new(Mutex::new(stream)),
                child,
            },
            audio: Some(audio),
            editor_events,
            editor_generation: 0,
            name: String::new(),
            latency: 0,
        };
        if let Some(e) = hello.error {
            return Err(e.into());
        }
        instrument.name = hello.name;
        instrument.latency = hello.latency;
        if let Some(state) = state {
            instrument.restore(state)?;
        }
        Ok(instrument)
    }

    fn kill(&self) {
        terminate(&self.child);
    }

    pub fn exited(&mut self) -> bool {
        self.child
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .try_wait()
            .map_or(true, |status| status.is_some())
    }

    pub fn call(&mut self, request: Request) -> Result<Reply> {
        if self.audio.as_ref().is_some_and(|audio| audio.failed) {
            self.kill();
            return Err("audio connection failed; recreate worker".into());
        }
        if self.audio.is_none()
            && !matches!(
                request,
                Request::FixtureFault { .. }
                    | Request::FixtureEdit { .. }
                    | Request::FixtureDelay { .. }
                    | Request::FixtureRestart
            )
        {
            return Err("stop live playback before this operation".into());
        }
        if let Request::Process { frames, events, .. } = &request {
            validate_block(*frames, events)?;
        }
        let result = (|| {
            if matches!(
                request,
                Request::Start { .. } | Request::Stop | Request::Restore { .. } | Request::Quit
            ) {
                self.audio
                    .as_mut()
                    .ok_or("stop live playback before lifecycle changes")?
                    .suspend()?;
            }
            if let Request::Process {
                position,
                events,
                frames,
                input,
                ..
            } = &request
            {
                let audio = self
                    .audio
                    .as_mut()
                    .ok_or("audio connection belongs to live playback")?;
                let deadline = Instant::now() + audio.timeout;
                audio.blocking(*position, events, *frames, input, deadline)
            } else {
                self.control
                    .call_until(&request, Instant::now() + control_timeout(&request))
            }
        })();
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                self.kill();
                return Err(error);
            }
        };
        if let Some(error) = reply.error.as_ref() {
            return Err(error.clone().into());
        }
        if reply.latency > crate::protocol::RATE * 10 {
            self.kill();
            return Err("worker latency exceeds prototype capacity".into());
        }
        self.latency = reply.latency;
        Ok(reply)
    }

    pub fn start(&mut self, offline: bool) -> Result<()> {
        if self.processing_mode == Some(offline) {
            if self.exited() {
                return Err("plugin worker exited".into());
            }
            return Ok(());
        }
        self.processing_mode = None;
        self.call(Request::Start { offline })?;
        self.processing_mode = Some(offline);
        let audio = self.audio.as_mut().ok_or("audio connection unavailable")?;
        audio.latency = self.latency;
        audio.timeout = Duration::from_secs(if offline { 60 } else { 1 });
        Ok(())
    }

    pub fn process(
        &mut self,
        position: u64,
        events: &[Midi],
        frames: usize,
    ) -> Result<(Vec<Frame>, bool)> {
        validate_block(frames, events)?;
        let reply = self.call(Request::Process {
            position,
            events: events.to_vec(),
            frames,
            input: Vec::new(),
            sequence: 0,
        })?;
        if reply.frames.len() != frames || reply.frames.iter().flatten().any(|v| !v.is_finite()) {
            self.kill();
            return Err("invalid or non-finite worker audio reply".into());
        }
        Ok((reply.frames, reply.restart))
    }

    pub fn control_connection(&self) -> ControlConnection {
        self.control.clone()
    }

    pub fn take_audio(&mut self) -> Result<AudioConnection> {
        self.audio
            .take()
            .ok_or_else(|| "audio connection already in use".into())
    }

    pub fn return_audio(&mut self, audio: AudioConnection) {
        self.latency = audio.latency;
        self.audio = Some(audio);
    }

    pub fn stop(&mut self) -> Result<()> {
        self.processing_mode = None;
        self.call(Request::Stop)?;
        Ok(())
    }
    pub fn editor(&mut self, open: bool) -> Result<()> {
        if open {
            // Drop notifications left by the previous editor session. The worker
            // advances its generation only after this open succeeds.
            let _ = self.editor_closed();
        }
        // A reply error such as "no editor" is a plugin property, not a broken
        // worker. Keep the child alive so the VI and its audio path remain usable.
        // A timed-out exchange terminates the worker inside `call_reply`.
        let request = Request::Editor { open };
        let reply = self
            .control
            .call_reply(&request, Instant::now() + control_timeout(&request))?;
        if let Some(error) = reply.error {
            return Err(error.into());
        }
        if reply.latency > crate::protocol::RATE * 10 {
            return Err("worker latency exceeds prototype capacity".into());
        }
        if open {
            // The worker sends this generation when the native window closes.
            // Advance it on success so the next close notification matches.
            self.editor_generation = self.editor_generation.wrapping_add(1);
        }
        Ok(())
    }
    pub fn editor_closed(&mut self) -> bool {
        let mut events = [0; 64];
        self.editor_events
            .read(&mut events)
            .is_ok_and(|read| events[..read].contains(&self.editor_generation))
    }
    pub fn save(&mut self) -> Result<Vec<u8>> {
        let state = self.call(Request::Save)?.state;
        if state.len() > MAX_STATE {
            return Err("plugin state exceeds limit".into());
        }
        Ok(state)
    }
    pub fn restore(&mut self, state: &[u8]) -> Result<()> {
        if state.len() > MAX_STATE {
            return Err("plugin state exceeds limit".into());
        }
        self.processing_mode = None;
        self.call(Request::Restore {
            state: state.to_vec(),
        })?;
        Ok(())
    }
}

/// One request sent on the audio connection whose reply has not been read yet.
struct InFlight {
    id: u64,
    /// The sequence the reply must echo: the request id for `Process`, zero for suspend.
    sequence: u64,
    frames: usize,
    sent: Instant,
}

/// The worker's audio connection, owned by one processing thread at a time.
///
/// Replies arrive in request order. A process block whose reply misses its deadline is
/// reported as late; the reply is read and discarded when it arrives, and a reply
/// interrupted mid-frame is resumed from the buffered bytes. Only a broken connection,
/// an invalid or out-of-sequence reply, or no reply for `timeout` fails the connection.
pub struct AudioConnection {
    stream: UnixStream,
    readiness: Readiness,
    /// Longest wait for any outstanding reply before the worker counts as stalled.
    timeout: Duration,
    pub latency: u32,
    failed: bool,
    next_id: u64,
    in_flight: VecDeque<InFlight>,
    outgoing: Vec<u8>,
    written: usize,
    incoming: Box<[u8]>,
    received: usize,
    // State from discarded late replies that the next delivered reply must carry.
    carried_restart: bool,
    carried_error: Option<String>,
}

impl AudioConnection {
    fn new(stream: UnixStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            readiness: Readiness::new()?,
            timeout: Duration::from_secs(1),
            latency: 0,
            failed: false,
            next_id: 1,
            in_flight: VecDeque::with_capacity(MAX_IN_FLIGHT + 1),
            outgoing: Vec::with_capacity(64 * 1024),
            written: 0,
            incoming: vec![0; 4 + MAX_AUDIO_PACKET].into_boxed_slice(),
            received: 0,
            carried_restart: false,
            carried_error: None,
        })
    }

    /// Process one instrument block, waiting up to the stall limit for its reply.
    pub fn process(&mut self, position: u64, events: &[Midi], frames: usize) -> Result<Reply> {
        validate_block(frames, events)?;
        let deadline = Instant::now() + self.timeout;
        let reply = self.blocking(position, events, frames, &[], deadline)?;
        match reply.error {
            Some(error) => Err(error.into()),
            None => Ok(reply),
        }
    }

    /// Process one insert block. `Ok(None)` means the reply missed `timeout` (or earlier
    /// blocks are still outstanding) and the caller should output dry for this block.
    pub fn process_effect(
        &mut self,
        position: u64,
        input: &[Frame],
        timeout: Duration,
    ) -> Result<Option<Reply>> {
        validate_block(input.len(), &[])?;
        if input.iter().flatten().any(|sample| !sample.is_finite()) {
            return Err("non-finite effect input".into());
        }
        let request = AudioRequest::Process {
            position,
            events: &[],
            frames: input.len(),
            input,
            sequence: self.next_id,
        };
        let reply = self.exchange(&request, input.len(), Instant::now() + timeout, false)?;
        match reply {
            Some(Reply {
                error: Some(error), ..
            }) => Err(error.into()),
            reply => Ok(reply),
        }
    }

    fn blocking(
        &mut self,
        position: u64,
        events: &[Midi],
        frames: usize,
        input: &[Frame],
        deadline: Instant,
    ) -> Result<Reply> {
        let request = AudioRequest::Process {
            position,
            events,
            frames,
            input,
            sequence: self.next_id,
        };
        self.exchange(&request, frames, deadline, true)?
            .ok_or_else(|| "plugin worker audio reply timed out".into())
    }

    /// Send `request` and wait until `deadline` for its reply. Returns `Ok(None)` when the
    /// block is late; with `block` set, lateness is a failure instead. Plugin-reported
    /// errors are returned inside the reply; every `Err` fails the connection.
    fn exchange(
        &mut self,
        request: &AudioRequest,
        frames: usize,
        deadline: Instant,
        block: bool,
    ) -> Result<Option<Reply>> {
        if self.failed {
            return Err("audio connection failed; recreate worker".into());
        }
        let result = self.exchange_inner(request, frames, deadline, block);
        match result {
            Ok(None) if block => Err(self.fail("plugin worker audio reply timed out")),
            Ok(reply) => Ok(reply),
            Err(error) => Err(self.fail(error)),
        }
    }

    fn fail(&mut self, error: impl Into<BoxError>) -> BoxError {
        self.failed = true;
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        error.into()
    }

    fn exchange_inner(
        &mut self,
        request: &AudioRequest,
        frames: usize,
        deadline: Instant,
        block: bool,
    ) -> Result<Option<Reply>> {
        // Drain replies of earlier late blocks and finish any interrupted request write.
        self.pump(None)?;
        while self.writing() || self.in_flight.len() >= MAX_IN_FLIGHT {
            if !block {
                return Ok(None);
            }
            if !self.wait_until(deadline)? {
                return Ok(None);
            }
            self.pump(None)?;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.outgoing.clear();
        self.outgoing.extend_from_slice(&[0; 4]);
        serde_json::to_writer(&mut self.outgoing, request)?;
        let length = self.outgoing.len() - 4;
        if length > MAX_AUDIO_PACKET {
            self.outgoing.clear();
            return Err("audio request exceeds limit".into());
        }
        self.outgoing[..4].copy_from_slice(&(length as u32).to_le_bytes());
        self.written = 0;
        self.in_flight.push_back(InFlight {
            id,
            sequence: if matches!(request, AudioRequest::Process { .. }) {
                id
            } else {
                0
            },
            frames,
            sent: Instant::now(),
        });
        loop {
            if let Some(reply) = self.pump(Some(id))? {
                return Ok(Some(reply));
            }
            if !self.wait_until(deadline)? {
                return Ok(None);
            }
        }
    }

    fn writing(&self) -> bool {
        self.written < self.outgoing.len()
    }

    /// Wait for readiness until `deadline`; false once the deadline has passed.
    fn wait_until(&self, deadline: Instant) -> Result<bool> {
        let Some(remaining) = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
        else {
            return Ok(false);
        };
        self.readiness
            .wait(self.stream.as_raw_fd(), self.writing(), remaining)?;
        Ok(true)
    }

    /// Write pending request bytes and read every complete reply without blocking.
    /// Returns the reply to request `awaited` once it arrives.
    fn pump(&mut self, awaited: Option<u64>) -> Result<Option<Reply>> {
        while self.writing() {
            match self.stream.write(&self.outgoing[self.written..]) {
                Ok(0) => return Err("worker audio connection closed".into()),
                Ok(written) => self.written += written,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        while let Some(reply) = self.read_reply()? {
            let expected = self
                .in_flight
                .pop_front()
                .ok_or("unsolicited worker audio reply")?;
            if reply.sequence != expected.sequence {
                return Err("worker audio reply out of sequence".into());
            }
            if Some(expected.id) == awaited {
                return self.deliver(reply, expected.frames).map(Some);
            }
            self.discard(reply, expected.frames)?;
        }
        if let Some(oldest) = self.in_flight.front()
            && oldest.sent.elapsed() >= self.timeout
        {
            return Err(format!(
                "plugin worker did not answer an audio request within {} ms",
                self.timeout.as_millis()
            )
            .into());
        }
        Ok(None)
    }

    /// Read one complete reply frame, keeping a partial frame buffered for the next call.
    fn read_reply(&mut self) -> Result<Option<Reply>> {
        loop {
            if self.received >= 4 {
                let header: [u8; 4] = self.incoming[..4].try_into().map_err(|_| "header")?;
                let length = u32::from_le_bytes(header) as usize;
                if length == 0 || length > MAX_AUDIO_PACKET {
                    return Err("invalid IPC packet length".into());
                }
                if self.received >= 4 + length {
                    let reply: Reply = serde_json::from_slice(&self.incoming[4..4 + length])?;
                    self.incoming.copy_within(4 + length..self.received, 0);
                    self.received -= 4 + length;
                    return Ok(Some(reply));
                }
            }
            // The buffer holds a whole maximal frame, so an incomplete frame always has room.
            match self.stream.read(&mut self.incoming[self.received..]) {
                Ok(0) => return Err("worker audio connection closed".into()),
                Ok(read) => self.received += read,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn validate(reply: &Reply, frames: usize) -> Result<()> {
        if reply.frames.len() != frames
            || reply.frames.iter().flatten().any(|v| !v.is_finite())
            || reply.latency > crate::protocol::RATE * 10
        {
            return Err("invalid worker audio reply".into());
        }
        Ok(())
    }

    /// Keep what a late reply reports that the processing thread must still act on.
    fn discard(&mut self, reply: Reply, frames: usize) -> Result<()> {
        if let Some(error) = reply.error {
            self.carried_error.get_or_insert(error);
            return Ok(());
        }
        Self::validate(&reply, frames)?;
        if reply.sequence != 0 {
            self.latency = reply.latency;
            self.carried_restart |= reply.restart;
        }
        Ok(())
    }

    fn deliver(&mut self, mut reply: Reply, frames: usize) -> Result<Reply> {
        if let Some(error) = self.carried_error.take() {
            reply.error = Some(error);
        }
        if reply.error.is_some() {
            return Ok(reply);
        }
        Self::validate(&reply, frames)?;
        if reply.sequence != 0 {
            reply.restart |= std::mem::take(&mut self.carried_restart);
            self.latency = reply.latency;
        }
        Ok(reply)
    }

    pub fn reconfigure(&mut self, control: &ControlConnection) -> Result<u32> {
        self.with_stopped(control, || Ok(()))?;
        Ok(self.latency)
    }

    pub fn with_stopped<T>(
        &mut self,
        control: &ControlConnection,
        action: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let result: Result<Result<T>> = (|| {
            let deadline = Instant::now() + Duration::from_secs(1);
            self.suspend()?;
            control.call_until(&Request::Stop, deadline)?;
            let value = action();
            let reply = control.call_until(
                &Request::Start { offline: false },
                Instant::now() + Duration::from_secs(1),
            )?;
            self.latency = reply.latency;
            // The restart reconfigured the plugin; late replies' restart requests are served.
            self.carried_restart = false;
            Ok(value)
        })();
        if result.is_err() {
            let _ = self.fail("audio lifecycle change failed");
        }
        result?
    }

    pub fn suspend(&mut self) -> Result<()> {
        let deadline = Instant::now() + self.timeout;
        let reply = self
            .exchange(&AudioRequest::SuspendAudio, 0, deadline, true)?
            .ok_or("plugin worker audio reply timed out")?;
        if let Some(error) = reply.error {
            return Err(error.into());
        }
        Ok(())
    }
}

impl Drop for Instrument {
    fn drop(&mut self) {
        // Never wait on a plugin during teardown; the owned Child prevents PID-reuse races.
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_failure_reply_survives_the_peer_closing() {
        let (mut tx, mut rx) = UnixStream::pair().unwrap();
        send(&mut tx, &Reply::failed("deliberate load failure")).unwrap();
        drop(tx);
        let mut deadline =
            DeadlineStream::new(&mut rx, Instant::now() + Duration::from_secs(60)).unwrap();
        let reply: Reply = receive(&mut deadline).unwrap();
        assert_eq!(reply.error.as_deref(), Some("deliberate load failure"));
    }

    #[test]
    fn blocked_writes_cannot_extend_the_whole_exchange_deadline() {
        let (mut tx, _rx) = UnixStream::pair().unwrap();
        let started = Instant::now();
        let mut stream =
            DeadlineStream::new(&mut tx, started + Duration::from_millis(100)).unwrap();
        let error = stream.write_all(&vec![0; 4 * 1024 * 1024]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn trickling_data_cannot_extend_the_whole_exchange_deadline() {
        let (mut tx, mut rx) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            for byte in [10, 0, 0, 0, 123, 32, 32, 32] {
                if tx.write_all(&[byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        let started = Instant::now();
        let mut stream =
            DeadlineStream::new(&mut rx, started + Duration::from_millis(100)).unwrap();
        assert!(receive::<Reply>(&mut stream).is_err());
        assert!(started.elapsed() < Duration::from_millis(220));
        drop(rx);
        writer.join().unwrap();
    }

    #[test]
    fn malformed_packet_is_rejected_before_allocation() {
        let (mut tx, mut rx) = UnixStream::pair().unwrap();
        tx.write_all(&u32::MAX.to_le_bytes()).unwrap();
        assert!(receive::<Request>(&mut rx).is_err());
    }

    #[test]
    fn editor_close_notification_matches_the_latest_open() {
        let (parent_control, mut worker_control) = UnixStream::pair().unwrap();
        let (parent_events, mut worker_events) = UnixStream::pair().unwrap();
        parent_events.set_nonblocking(true).unwrap();
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let worker = std::thread::spawn(move || {
            let request: Request = receive(&mut worker_control).unwrap();
            assert!(matches!(request, Request::Editor { open: true }));
            send(&mut worker_control, &Reply::empty()).unwrap();
            // The worker advances its generation after a successful open, then
            // reports that generation when the native window closes.
            worker_events.write_all(&[1]).unwrap();
        });
        let child = Arc::new(Mutex::new(child));
        let mut instrument = Instrument {
            processing_mode: None,
            child: Arc::clone(&child),
            control: ControlConnection {
                stream: Arc::new(Mutex::new(parent_control)),
                child,
            },
            audio: None,
            editor_events: parent_events,
            editor_generation: 0,
            name: String::new(),
            latency: 0,
        };
        instrument.editor(true).unwrap();
        worker.join().unwrap();
        let started = Instant::now();
        while !instrument.editor_closed() {
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "close notification was ignored"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn process_reply(request: &Request) -> Reply {
        let Request::Process {
            input, sequence, ..
        } = request
        else {
            panic!("unexpected audio request {request:?}");
        };
        Reply {
            frames: input.iter().map(|f| [f[0] * 0.5, f[1] * 0.5]).collect(),
            sequence: *sequence,
            ..Reply::empty()
        }
    }

    fn block(value: f32) -> Vec<Frame> {
        vec![[value, -value]; 8]
    }

    #[test]
    fn late_effect_block_is_dry_and_its_reply_resumes_mid_frame_then_is_discarded() {
        let (host, mut worker) = UnixStream::pair().unwrap();
        let fake = std::thread::spawn(move || {
            let first: Request = receive_audio(&mut worker).unwrap();
            let mut late = process_reply(&first);
            late.restart = true;
            let data = serde_json::to_vec(&late).unwrap();
            let mut packet = (data.len() as u32).to_le_bytes().to_vec();
            packet.extend_from_slice(&data);
            // Interrupt the late reply mid-frame, past the host's block deadline.
            worker.write_all(&packet[..packet.len() / 2]).unwrap();
            std::thread::sleep(Duration::from_millis(40));
            worker.write_all(&packet[packet.len() / 2..]).unwrap();
            let second: Request = receive_audio(&mut worker).unwrap();
            send(&mut worker, &process_reply(&second)).unwrap();
        });
        let mut audio = AudioConnection::new(host).unwrap();
        let started = Instant::now();
        assert!(
            audio
                .process_effect(0, &block(0.5), Duration::from_millis(5))
                .unwrap()
                .is_none()
        );
        assert!(started.elapsed() < Duration::from_millis(30));
        std::thread::sleep(Duration::from_millis(60));
        let reply = audio
            .process_effect(8, &block(0.25), Duration::from_millis(500))
            .unwrap()
            .expect("the next block is answered");
        assert_eq!(reply.frames, vec![[0.125, -0.125]; 8]);
        assert!(reply.restart, "a discarded reply's restart request is kept");
        assert!(!audio.failed);
        fake.join().unwrap();
    }

    #[test]
    fn outstanding_blocks_are_bounded_and_only_a_sustained_stall_fails() {
        let (host, mut worker) = UnixStream::pair().unwrap();
        let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&received);
        let fake = std::thread::spawn(move || {
            while receive_audio::<Request>(&mut worker).is_ok() {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        let mut audio = AudioConnection::new(host).unwrap();
        audio.timeout = Duration::from_millis(200);
        let started = Instant::now();
        for index in 0..6 {
            let late = audio
                .process_effect(index * 8, &block(0.5), Duration::from_millis(1))
                .unwrap();
            assert!(late.is_none());
        }
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            received.load(std::sync::atomic::Ordering::SeqCst),
            MAX_IN_FLIGHT
        );
        assert!(!audio.failed);
        std::thread::sleep(Duration::from_millis(200).saturating_sub(started.elapsed()));
        assert!(
            audio
                .process_effect(48, &block(0.5), Duration::from_millis(1))
                .is_err()
        );
        assert!(audio.failed);
        fake.join().unwrap();
    }

    #[test]
    fn out_of_sequence_audio_reply_fails_the_connection() {
        let (host, mut worker) = UnixStream::pair().unwrap();
        let fake = std::thread::spawn(move || {
            let request: Request = receive_audio(&mut worker).unwrap();
            let mut reply = process_reply(&request);
            reply.sequence += 7;
            send(&mut worker, &reply).unwrap();
        });
        let mut audio = AudioConnection::new(host).unwrap();
        assert!(
            audio
                .process_effect(0, &block(0.5), Duration::from_millis(500))
                .is_err()
        );
        assert!(audio.failed);
        fake.join().unwrap();
    }

    #[test]
    fn audio_waits_do_not_round_up_to_whole_milliseconds() {
        let (host, _worker) = UnixStream::pair().unwrap();
        let readiness = Readiness::new().unwrap();
        let wait = Duration::from_micros(300);
        let fastest = (0..10)
            .map(|_| {
                let started = Instant::now();
                readiness.wait(host.as_raw_fd(), false, wait).unwrap();
                started.elapsed()
            })
            .min()
            .unwrap();
        // The previous whole-millisecond poll waited at least 1 ms for this deadline.
        assert!(fastest < Duration::from_micros(700), "{fastest:?}");
    }

    #[test]
    fn control_timeout_terminates_the_worker_so_a_stale_reply_is_never_read() {
        let (parent, mut worker) = UnixStream::pair().unwrap();
        let child = Arc::new(Mutex::new(Command::new("sleep").arg("30").spawn().unwrap()));
        let control = ControlConnection {
            stream: Arc::new(Mutex::new(parent)),
            child: Arc::clone(&child),
        };
        let fake = std::thread::spawn(move || {
            let _: Request = receive(&mut worker).unwrap();
            std::thread::sleep(Duration::from_millis(100));
            // The stale answer arrives after the host gave up.
            let _ = send(&mut worker, &Reply::empty());
            let _: Result<Request> = receive(&mut worker);
        });
        let started = Instant::now();
        assert!(
            control
                .call_reply(&Request::Stop, started + Duration::from_millis(50))
                .is_err()
        );
        assert!(child.lock().unwrap().try_wait().unwrap().is_some());
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            control
                .call_reply(&Request::Stop, Instant::now() + Duration::from_millis(500))
                .is_err(),
            "a later call must not read the stale reply"
        );
        fake.join().unwrap();
    }

    #[test]
    fn only_load_editor_open_and_restore_keep_long_control_limits() {
        assert_eq!(
            control_timeout(&Request::Restore { state: Vec::new() }),
            LONG_CONTROL
        );
        assert_eq!(
            control_timeout(&Request::Editor { open: true }),
            LONG_CONTROL
        );
        assert_eq!(control_timeout(&Request::Save), SAVE_CONTROL);
        for request in [
            Request::Start { offline: false },
            Request::Stop,
            Request::Editor { open: false },
            Request::Quit,
        ] {
            assert_eq!(control_timeout(&request), SHORT_CONTROL);
        }
    }
}
