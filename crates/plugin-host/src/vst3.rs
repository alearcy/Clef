use crate::protocol::{Frame, MAX_STATE, Midi, Result, validate_block};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
    ptr::NonNull,
    sync::Arc,
    thread::ThreadId,
};

#[repr(C)]
struct NativeMidi {
    offset: u32,
    bytes: [u8; 3],
}

unsafe extern "C" {
    fn clef_vst_error() -> *const c_char;
    fn clef_vst_load(
        path: *const c_char,
        selected: *const c_char,
        effect: i32,
        out: *mut *mut c_void,
    ) -> i32;
    fn clef_vst_destroy(plugin: *mut c_void) -> i32;
    fn clef_vst_name(plugin: *mut c_void) -> *const c_char;
    fn clef_vst_activate(plugin: *mut c_void, offline: i32, rate: f64, latency: *mut u32) -> i32;
    fn clef_vst_deactivate(plugin: *mut c_void) -> i32;
    fn clef_vst_process(
        plugin: *mut c_void,
        position: u64,
        events: *const NativeMidi,
        count: usize,
        frames: usize,
        input: *const f32,
        output: *mut f32,
    ) -> i32;
    fn clef_vst_suspend(plugin: *mut c_void) -> i32;
    fn clef_vst_editor(
        plugin: *mut c_void,
        parent: *mut c_void,
        open: i32,
        width: *mut i32,
        height: *mut i32,
        resizable: *mut i32,
    ) -> i32;
    fn clef_vst_resize(plugin: *mut c_void, width: *mut i32, height: *mut i32) -> i32;
    fn clef_vst_poll(
        plugin: *mut c_void,
        flags: *mut u32,
        width: *mut i32,
        height: *mut i32,
    ) -> i32;
    fn clef_vst_save(plugin: *mut c_void, bytes: *mut *const u8, size: *mut usize) -> i32;
    fn clef_vst_restore(plugin: *mut c_void, bytes: *const u8, size: usize) -> i32;
}

fn check(result: i32) -> Result<()> {
    if result == 1 {
        return Ok(());
    }
    // Every failed ABI call sets an error in the calling thread's fixed buffer.
    Err(unsafe { CStr::from_ptr(clef_vst_error()) }
        .to_string_lossy()
        .into_owned()
        .into())
}

struct Owner {
    pointer: NonNull<c_void>,
    main: ThreadId,
}
// The ABI separates main-thread and audio fields; only signal queues/atomics cross them.
unsafe impl Send for Owner {}
unsafe impl Sync for Owner {}
impl Drop for Owner {
    fn drop(&mut self) {
        if std::thread::current().id() != self.main {
            std::process::abort();
        }
        if let Err(error) = check(unsafe { clef_vst_destroy(self.pointer.as_ptr()) }) {
            eprintln!("VST3 teardown failed: {error}");
        }
    }
}

pub struct VstMain {
    owner: Arc<Owner>,
    pub name: String,
    pub latency: u32,
    pub resizable: bool,
    sample_rate: u32,
    _main_only: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl VstMain {
    pub fn load(path: &Path, selected: &str, sample_rate: u32, effect: bool) -> Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let path = CString::new(path.as_os_str().as_bytes())?;
        let selected = CString::new(selected)?;
        let mut pointer = std::ptr::null_mut();
        check(unsafe {
            clef_vst_load(
                path.as_ptr(),
                selected.as_ptr(),
                effect.into(),
                &mut pointer,
            )
        })?;
        let pointer = NonNull::new(pointer).ok_or("SDK returned null plugin")?;
        let owner = Arc::new(Owner {
            pointer,
            main: std::thread::current().id(),
        });
        let name = unsafe { CStr::from_ptr(clef_vst_name(pointer.as_ptr())) }
            .to_string_lossy()
            .into_owned();
        Ok(Self {
            owner,
            name,
            latency: 0,
            resizable: false,
            sample_rate,
            _main_only: Default::default(),
        })
    }
    fn pointer(&self) -> *mut c_void {
        self.owner.pointer.as_ptr()
    }
    pub fn activate(&mut self, offline: bool) -> Result<VstAudio> {
        if Arc::strong_count(&self.owner) != 1 {
            return Err("VST3 audio handle already exists".into());
        }
        check(unsafe {
            clef_vst_activate(
                self.pointer(),
                offline.into(),
                self.sample_rate.into(),
                &mut self.latency,
            )
        })?;
        Ok(VstAudio {
            owner: Arc::clone(&self.owner),
        })
    }
    pub fn deactivate(&mut self) -> Result<()> {
        if Arc::strong_count(&self.owner) != 1 {
            return Err("return VST3 audio handle before deactivation".into());
        }
        check(unsafe { clef_vst_deactivate(self.pointer()) })
    }
    pub fn editor(&mut self, parent: *mut c_void, open: bool) -> Result<(u32, u32)> {
        let (mut w, mut h, mut resizable) = (0, 0, 0);
        check(unsafe {
            clef_vst_editor(
                self.pointer(),
                parent,
                open.into(),
                &mut w,
                &mut h,
                &mut resizable,
            )
        })?;
        self.resizable = resizable != 0;
        Ok((w.try_into()?, h.try_into()?))
    }
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(u32, u32)> {
        let (mut w, mut h) = (i32::try_from(width)?, i32::try_from(height)?);
        check(unsafe { clef_vst_resize(self.pointer(), &mut w, &mut h) })?;
        Ok((w.try_into()?, h.try_into()?))
    }
    pub fn service(&mut self) -> Result<(u32, Option<(u32, u32)>)> {
        let (mut flags, mut w, mut h) = (0, 0, 0);
        check(unsafe { clef_vst_poll(self.pointer(), &mut flags, &mut w, &mut h) })?;
        let size = if w > 0 && h > 0 {
            Some((w.try_into()?, h.try_into()?))
        } else {
            None
        };
        Ok((flags, size))
    }
    pub fn save(&mut self) -> Result<Vec<u8>> {
        let (mut bytes, mut size) = (std::ptr::null(), 0);
        check(unsafe { clef_vst_save(self.pointer(), &mut bytes, &mut size) })?;
        if size > MAX_STATE || bytes.is_null() {
            return Err("invalid SDK state output".into());
        }
        // The main-thread ABI keeps this snapshot alive until the next save or destruction.
        Ok(unsafe { std::slice::from_raw_parts(bytes, size) }.to_vec())
    }
    pub fn restore(&mut self, state: &[u8]) -> Result<()> {
        if state.len() > MAX_STATE {
            return Err("state exceeds limit".into());
        }
        check(unsafe { clef_vst_restore(self.pointer(), state.as_ptr(), state.len()) })
    }
}

pub struct VstAudio {
    owner: Arc<Owner>,
}
impl VstAudio {
    pub fn process(
        &mut self,
        position: u64,
        events: &[Midi],
        frames: usize,
        input: &[Frame],
    ) -> Result<Vec<Frame>> {
        validate_block(frames, events)?;
        if !input.is_empty() && input.len() != frames {
            return Err("invalid VST3 input length".into());
        }
        let midi: Vec<_> = events
            .iter()
            .map(|m| NativeMidi {
                offset: m.offset,
                bytes: m.bytes,
            })
            .collect();
        let mut output = vec![[0.0; 2]; frames];
        check(unsafe {
            clef_vst_process(
                self.owner.pointer.as_ptr(),
                position,
                midi.as_ptr(),
                midi.len(),
                frames,
                if input.is_empty() {
                    std::ptr::null()
                } else {
                    input.as_ptr().cast()
                },
                output.as_mut_ptr().cast(),
            )
        })?;
        Ok(output)
    }
    pub fn stop(&mut self) -> Result<()> {
        check(unsafe { clef_vst_suspend(self.owner.pointer.as_ptr()) })
    }
}
