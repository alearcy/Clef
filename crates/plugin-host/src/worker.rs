use crate::{
    clap_host::{ClapAudio, ClapMain},
    native::NativeWindow,
    protocol::{Frame, MAX_STATE, Midi, RATE, Reply, Request, Result, validate_block},
    rpc,
    vst3::{VstAudio, VstMain},
};
use clack_extensions::gui::{GuiApiType, GuiConfiguration, GuiSize, Window as ClapWindow};
use std::{
    os::unix::net::UnixStream,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};

enum Audio {
    Vst(VstAudio),
    Clap(ClapAudio),
    Fixture(Arc<AtomicU8>),
}

impl Audio {
    fn stop(&mut self) -> Result<()> {
        match self {
            Self::Vst(audio) => audio.stop()?,
            Self::Clap(audio) => audio.stop()?,
            Self::Fixture(_) => {}
        }
        Ok(())
    }
    fn process(&mut self, position: u64, events: &[Midi], frames: usize) -> Result<Vec<Frame>> {
        validate_block(frames, events)?;
        match self {
            Self::Vst(audio) => audio.process(position, events, frames),
            Self::Clap(audio) => audio.process(position, events, frames),
            Self::Fixture(fault) => {
                match fault.load(Ordering::Acquire) {
                    1 => std::process::exit(91),
                    2 => loop {
                        std::thread::park();
                    },
                    _ => {}
                }
                let mut output = vec![[0.0; 2]; frames];
                for event in events {
                    if event.bytes[0] & 0xf0 == 0x90 && event.bytes[2] > 0 {
                        output[event.offset as usize] = [0.25; 2];
                    }
                }
                Ok(output)
            }
        }
    }
}

enum Backend {
    Vst(VstMain),
    Clap(Box<ClapMain>),
    Fixture(Arc<AtomicU8>),
}

struct MainPlugin {
    backend: Backend,
    audio: Arc<Mutex<Option<Audio>>>,
    window: NativeWindow,
    editor_open: bool,
    name: String,
    fixture_state: u8,
    latency: Arc<AtomicU32>,
    restart: Arc<AtomicBool>,
    suspended: Arc<AtomicBool>,
    realtime: Arc<AtomicBool>,
    activity: Option<crate::threading::Activity>,
}

impl MainPlugin {
    fn load(path: &Path, window: NativeWindow, selected: &str, sample_rate: u32) -> Result<Self> {
        let (backend, name) = if path == Path::new("@fixture") {
            (
                Backend::Fixture(Arc::new(AtomicU8::new(0))),
                "Clef deterministic process fixture".to_owned(),
            )
        } else if path.extension().is_some_and(|s| s == "vst3") {
            let plugin = VstMain::load(path, selected, sample_rate)?;
            let name = plugin.name.clone();
            (Backend::Vst(plugin), name)
        } else if path.extension().is_some_and(|s| s == "clap") {
            let plugin = ClapMain::load(path, selected, sample_rate)?;
            let name = plugin.name.clone();
            (Backend::Clap(Box::new(plugin)), name)
        } else {
            return Err("expected a .vst3 or .clap bundle".into());
        };
        let mut main = Self {
            backend,
            audio: Arc::new(Mutex::new(None)),
            window,
            editor_open: false,
            name,
            fixture_state: 0,
            latency: Arc::new(AtomicU32::new(0)),
            restart: Arc::new(AtomicBool::new(false)),
            suspended: Arc::new(AtomicBool::new(true)),
            realtime: Arc::new(AtomicBool::new(false)),
            activity: None,
        };
        main.update_latency()?;
        Ok(main)
    }

    fn update_latency(&mut self) -> Result<()> {
        let latency = match &mut self.backend {
            Backend::Vst(plugin) => plugin.latency,
            Backend::Clap(plugin) => plugin
                .latency()
                .unwrap_or_else(|| self.latency.load(Ordering::Acquire)),
            Backend::Fixture(_) => 0,
        };
        if latency > RATE * 10 {
            return Err("plugin latency exceeds prototype capacity".into());
        }
        self.latency.store(latency, Ordering::Release);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        let audio = self.audio.lock().map_err(|_| "audio lock poisoned")?.take();
        match (&mut self.backend, audio) {
            (Backend::Clap(main), Some(Audio::Clap(audio))) => main.deactivate(audio),
            (Backend::Vst(plugin), audio) => {
                drop(audio);
                plugin.deactivate()?;
            }
            _ => {}
        }
        self.activity.take();
        Ok(())
    }

    fn control(&mut self, request: Request) -> Result<Reply> {
        let mut reply = Reply::empty();
        if matches!(
            request,
            Request::Start { .. } | Request::Stop | Request::Restore { .. } | Request::Quit
        ) && !self.suspended.load(Ordering::Acquire)
        {
            return Err(
                "audio must be suspended on its processing thread before lifecycle changes".into(),
            );
        }
        match request {
            Request::Start { offline } => {
                self.stop()?;
                self.activity = (!offline)
                    .then(crate::threading::Activity::begin)
                    .transpose()?;
                self.realtime.store(!offline, Ordering::Release);
                let audio = match &mut self.backend {
                    Backend::Vst(plugin) => Audio::Vst(plugin.activate(offline)?),
                    Backend::Clap(main) => Audio::Clap(main.activate(offline)?),
                    Backend::Fixture(fault) => Audio::Fixture(Arc::clone(fault)),
                };
                *self.audio.lock().map_err(|_| "audio lock poisoned")? = Some(audio);
                self.restart.store(false, Ordering::Release);
            }
            Request::Stop => self.stop()?,
            Request::Editor { open } => self.editor(open)?,
            Request::FixtureEdit { value } => match &self.backend {
                Backend::Fixture(_) => self.fixture_state = value,
                _ => return Err("fixture edits are restricted to the internal fixture".into()),
            },
            Request::FixtureFault { hang } => match &self.backend {
                Backend::Fixture(fault) => fault.store(if hang { 2 } else { 1 }, Ordering::Release),
                _ => return Err("fault injection is restricted to the internal fixture".into()),
            },
            Request::Save => {
                if self
                    .audio
                    .lock()
                    .map_err(|_| "audio lock poisoned")?
                    .is_some()
                {
                    return Err("stop playback before saving state".into());
                }
                reply.state = match &mut self.backend {
                    Backend::Vst(plugin) => plugin.save()?,
                    Backend::Clap(plugin) => plugin.save()?,
                    Backend::Fixture(_) => vec![self.fixture_state],
                };
            }
            Request::Restore { state } => {
                if state.len() > MAX_STATE {
                    return Err("state exceeds limit".into());
                }
                self.stop()?;
                match &mut self.backend {
                    Backend::Vst(plugin) => plugin.restore(&state)?,
                    Backend::Clap(plugin) => plugin.restore(&state)?,
                    Backend::Fixture(_) => {
                        if state.len() != 1 {
                            return Err("invalid fixture state".into());
                        }
                        self.fixture_state = state[0];
                    }
                }
            }
            Request::Quit => {
                self.stop()?;
                self.editor(false)?;
            }
            Request::Process { .. } | Request::SuspendAudio => {
                return Err("process request reached the control thread".into());
            }
        }
        self.update_latency()?;
        reply.latency = self.latency.load(Ordering::Acquire);
        Ok(reply)
    }

    fn editor(&mut self, open: bool) -> Result<()> {
        if !open {
            self.window.detach_parent();
            match &mut self.backend {
                Backend::Vst(plugin) => {
                    plugin.editor(std::ptr::null_mut(), false)?;
                }
                Backend::Clap(plugin) if plugin.gui_open => {
                    if let Some(gui) = plugin.gui {
                        gui.destroy(&plugin.instance.plugin_handle());
                    }
                    plugin.gui_open = false;
                    plugin.signals.resize.store(0, Ordering::Release);
                    plugin.signals.show.store(false, Ordering::Release);
                    plugin.signals.hide.store(false, Ordering::Release);
                    plugin.signals.closed.store(false, Ordering::Release);
                }
                _ => {}
            }
            self.window.destroy_parent();
            self.editor_open = false;
            self.window.hide();
            return Ok(());
        }
        self.editor(false)?;
        let result: Result<()> = (|| {
            self.window.attach_parent()?;
            match &mut self.backend {
                Backend::Vst(plugin) => {
                    let (w, h) = plugin.editor(self.window.view_ptr(), true)?;
                    self.window.resize(w, h)?;
                    self.window.resizable(plugin.resizable);
                    self.window.show();
                }
                Backend::Clap(plugin) => {
                    let gui = plugin.gui.ok_or("CLAP plugin has no editor")?;
                    let handle = plugin.instance.plugin_handle();
                    let config = GuiConfiguration {
                        api_type: GuiApiType::COCOA,
                        is_floating: false,
                    };
                    if !gui.is_api_supported(&handle, config) {
                        return Err("CLAP Cocoa embedding is unsupported".into());
                    }
                    gui.create(&handle, config)?;
                    plugin.gui_open = true;
                    let size = gui
                        .get_size(&handle)
                        .ok_or("CLAP editor did not report a size")?;
                    self.window.resize(size.width, size.height)?;
                    // The worker's main thread owns the NSView and destroys the GUI before the window.
                    unsafe {
                        gui.set_parent(
                            &handle,
                            ClapWindow::from_cocoa_nsview(self.window.view_ptr()),
                        )?;
                    }
                    self.window.resizable(gui.can_resize(&handle));
                    gui.show(&handle)?;
                    self.window.show();
                }
                Backend::Fixture(_) => return Err("fixture has no editor".into()),
            }
            Ok(())
        })();
        if result.is_err() {
            self.editor(false)?;
        } else {
            self.editor_open = true;
        }
        result
    }

    fn resize_from_host(&mut self, size: eframe::egui::Vec2) -> Result<()> {
        if !self.editor_open {
            return Ok(());
        }
        let (width, height) = (size.x.round() as u32, size.y.round() as u32);
        crate::native::validate_size(width, height)?;
        match &mut self.backend {
            Backend::Vst(plugin) => {
                if plugin.resizable {
                    let (w, h) = plugin.resize(width, height)?;
                    self.window.resize(w, h)?;
                }
            }
            Backend::Clap(plugin) => {
                if let Some(gui) = plugin.gui {
                    let handle = plugin.instance.plugin_handle();
                    if gui.can_resize(&handle) {
                        if gui.get_size(&handle) == Some(GuiSize { width, height }) {
                            return Ok(());
                        }
                        let size = gui
                            .adjust_size(&handle, GuiSize { width, height })
                            .ok_or("CLAP rejected editor resize")?;
                        gui.set_size(&handle, size).map_err(|error| {
                            format!(
                                "CLAP host resize to {}x{} (open={}): {error}",
                                size.width, size.height, plugin.gui_open
                            )
                        })?;
                        self.window.resize(size.width, size.height)?;
                    }
                }
            }
            Backend::Fixture(_) => {}
        }
        Ok(())
    }

    fn service(&mut self) -> Result<()> {
        match &mut self.backend {
            Backend::Vst(plugin) => {
                let (flags, size) = plugin.service()?;
                if flags & 0x0b != 0 {
                    self.restart.store(true, Ordering::Release);
                }
                if let Some((w, h)) = size {
                    self.window.resize(w, h)?;
                }
            }
            Backend::Clap(plugin) => {
                plugin.service();
                if plugin.signals.restart.load(Ordering::Acquire) {
                    self.restart.store(true, Ordering::Release);
                }
                let size = plugin.signals.resize.swap(0, Ordering::AcqRel);
                if size != 0 {
                    let size = GuiSize {
                        width: (size >> 32) as u32,
                        height: size as u32,
                    };
                    // A plugin-originated request already resized its GUI; accept the host area.
                    if plugin.gui_open {
                        self.window.resize(size.width, size.height)?;
                    }
                }
                if plugin.signals.show.swap(false, Ordering::AcqRel) && plugin.gui_open {
                    if let Some(gui) = plugin.gui {
                        gui.show(&plugin.instance.plugin_handle())?;
                    }
                    self.window.show();
                }
                if plugin.signals.hide.swap(false, Ordering::AcqRel) && plugin.gui_open {
                    if let Some(gui) = plugin.gui {
                        gui.hide(&plugin.instance.plugin_handle())?;
                    }
                    self.window.hide();
                }
                if plugin.signals.closed.swap(false, Ordering::AcqRel) {
                    self.editor(false)?;
                }
            }
            Backend::Fixture(_) => {}
        }
        Ok(())
    }
}

pub fn run(socket: &Path, path: &Path, selected: String, sample_rate: u32) -> Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    let mut audio_stream = UnixStream::connect(socket)?;
    let editor_events = UnixStream::connect(socket)?;
    editor_events.set_nonblocking(true)?;
    let path = path.to_owned();
    let outcome = std::rc::Rc::new(std::cell::RefCell::new(None));
    let app_outcome = outcome.clone();
    let options = eframe::NativeOptions {
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        })),
        viewport: eframe::egui::ViewportBuilder::default()
            .with_visible(false)
            .with_resizable(false)
            .with_inner_size([640.0, 480.0])
            .with_clamp_size_to_monitor_size(false),
        ..Default::default()
    };
    eframe::run_native(
        "Clef Plugin Editor",
        options,
        Box::new(move |cc| {
            let mut window = NativeWindow::new(cc.egui_ctx.clone(), cc)?;
            // eframe can reveal the root window after its first paint. Viewport
            // commands run afterward, so keep the empty window out of sight.
            window.hide();
            let main = match MainPlugin::load(&path, window, &selected, sample_rate) {
                Ok(main) => main,
                Err(error) => {
                    rpc::send(&mut stream, &Reply::failed(&error))?;
                    return Err(error);
                }
            };
            let mut hello = Reply::empty();
            hello.name = main.name.clone();
            hello.latency = main.latency.load(Ordering::Acquire);
            rpc::send(&mut stream, &hello)?;
            let audio = Arc::clone(&main.audio);
            let latency = Arc::clone(&main.latency);
            let restart = Arc::clone(&main.restart);
            let done = Arc::new(AtomicBool::new(false));
            let reader_done = Arc::clone(&done);
            let (tx, rx) = mpsc::sync_channel::<(Request, mpsc::SyncSender<Reply>)>(1);
            let shutdown = stream.try_clone()?;
            let audio_shutdown = audio_stream.try_clone()?;
            let audio_done = Arc::clone(&done);
            let suspended = Arc::clone(&main.suspended);
            let realtime = Arc::clone(&main.realtime);
            let processor = std::thread::spawn(move || {
                let mut high_priority = false;
                while let Ok(request) = rpc::receive_audio::<Request>(&mut audio_stream) {
                    let response = match request {
                        Request::Process {
                            position,
                            events,
                            frames,
                        } => {
                            let started = std::time::Instant::now();
                            let result: Result<Vec<Frame>> = (|| {
                                let requested = realtime.load(Ordering::Acquire);
                                if requested != high_priority {
                                    crate::threading::audio_priority(requested)?;
                                    high_priority = requested;
                                }
                                let mut audio = audio.lock().map_err(|_| "audio lock poisoned")?;
                                suspended.store(false, Ordering::Release);
                                audio
                                    .as_mut()
                                    .ok_or("plugin is stopped")?
                                    .process(position, &events, frames)
                            })();
                            match result {
                                Ok(frames) => Reply {
                                    frames,
                                    processing_nanos: started
                                        .elapsed()
                                        .as_nanos()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                    latency: latency.load(Ordering::Acquire),
                                    restart: restart.swap(false, Ordering::AcqRel),
                                    ..Reply::empty()
                                },
                                Err(error) => Reply::failed(error),
                            }
                        }
                        Request::SuspendAudio => match audio.lock() {
                            Ok(mut audio) => {
                                if let Some(audio) = audio.as_mut()
                                    && let Err(error) = audio.stop()
                                {
                                    let _ = rpc::send(&mut audio_stream, &Reply::failed(error));
                                    break;
                                }
                                suspended.store(true, Ordering::Release);
                                Reply::empty()
                            }
                            Err(_) => Reply::failed("audio lock poisoned"),
                        },
                        _ => Reply::failed("control request on audio connection"),
                    };
                    if rpc::send(&mut audio_stream, &response).is_err() {
                        break;
                    }
                }
                if let Ok(mut audio) = audio.lock()
                    && let Some(audio) = audio.as_mut()
                    && let Err(error) = audio.stop()
                {
                    eprintln!("Worker audio stop failed: {error}");
                }
                suspended.store(true, Ordering::Release);
                audio_done.store(true, Ordering::Release);
            });
            let reader = std::thread::spawn(move || {
                while let Ok(request) = rpc::receive::<Request>(&mut stream) {
                    let quit = matches!(request, Request::Quit);
                    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
                    if tx.send((request, reply_tx)).is_err() {
                        break;
                    }
                    let response = loop {
                        match reply_rx.recv_timeout(Duration::from_millis(20)) {
                            Ok(reply) => break reply,
                            Err(mpsc::RecvTimeoutError::Timeout)
                                if !reader_done.load(Ordering::Acquire) =>
                            {
                                continue;
                            }
                            Err(_) => return,
                        }
                    };
                    if rpc::send(&mut stream, &response).is_err() || quit {
                        break;
                    }
                }
                reader_done.store(true, Ordering::Release);
            });
            Ok(Box::new(WorkerApp {
                main,
                done,
                rx,
                reader: Some(reader),
                processor: Some(processor),
                shutdown,
                audio_shutdown,
                editor_events,
                editor_generation: 0,
                outcome: app_outcome,
                exiting: false,
                last_size: None,
            }))
        }),
    )
    .map_err(|error| error.to_string())?;
    if let Some(error) = outcome.borrow_mut().take() {
        return Err(error.into());
    }
    Ok(())
}

struct WorkerApp {
    main: MainPlugin,
    done: Arc<AtomicBool>,
    rx: mpsc::Receiver<(Request, mpsc::SyncSender<Reply>)>,
    reader: Option<std::thread::JoinHandle<()>>,
    processor: Option<std::thread::JoinHandle<()>>,
    shutdown: UnixStream,
    audio_shutdown: UnixStream,
    editor_events: UnixStream,
    editor_generation: u8,
    outcome: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    exiting: bool,
    last_size: Option<eframe::egui::Vec2>,
}

impl eframe::App for WorkerApp {
    fn logic(&mut self, ctx: &eframe::egui::Context, _: &mut eframe::Frame) {
        if ctx.input(|input| input.viewport().close_requested()) && !self.exiting {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
            let was_open = self.main.editor_open;
            if let Err(error) = self.main.editor(false) {
                *self.outcome.borrow_mut() = Some(error.to_string());
                self.done.store(true, Ordering::Release);
            } else if was_open {
                self.notify_editor_closed();
            }
        }
        let size = ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.size()));
        if self.main.window.is_visible()
            && size != self.last_size
            && let Some(size) = size
            && let Err(error) = self.main.resize_from_host(size)
        {
            *self.outcome.borrow_mut() = Some(error.to_string());
            self.done.store(true, Ordering::Release);
        }
        self.last_size = size;
        while let Ok((request, reply)) = self.rx.try_recv() {
            let opening = matches!(&request, Request::Editor { open: true });
            let response = self.main.control(request).unwrap_or_else(Reply::failed);
            if opening && response.error.is_none() {
                self.editor_generation = self.editor_generation.wrapping_add(1);
            }
            let _ = reply.send(response);
        }
        let was_open = self.main.editor_open;
        if let Err(error) = self.main.service() {
            *self.outcome.borrow_mut() = Some(error.to_string());
            self.done.store(true, Ordering::Release);
        }
        if was_open && !self.main.editor_open {
            self.notify_editor_closed();
        }
        if self.done.load(Ordering::Acquire) {
            self.exiting = true;
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
        } else {
            self.main.window.present();
            ctx.request_repaint_after(Duration::from_millis(2));
        }
    }

    fn ui(&mut self, _: &mut eframe::egui::Ui, _: &mut eframe::Frame) {}

    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.done.store(true, Ordering::Release);
        let _ = self.shutdown.shutdown(std::net::Shutdown::Both);
        let _ = self.audio_shutdown.shutdown(std::net::Shutdown::Both);
        if let Some(processor) = self.processor.take() {
            let _ = processor.join();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        for result in [self.main.stop(), self.main.editor(false)] {
            if let Err(error) = result {
                *self.outcome.borrow_mut() = Some(error.to_string());
            }
        }
    }
}

impl WorkerApp {
    fn notify_editor_closed(&mut self) {
        use std::io::Write;
        let _ = self.editor_events.write(&[self.editor_generation]);
    }
}
