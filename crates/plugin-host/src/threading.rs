use crate::protocol::Result;

unsafe extern "C" {
    fn clef_audio_thread_priority(realtime: i32) -> i32;
    fn clef_audio_activity_begin() -> *mut std::ffi::c_void;
    fn clef_audio_activity_end(activity: *mut std::ffi::c_void);
    #[cfg(test)]
    fn clef_audio_thread_is_realtime() -> i32;
}

pub fn audio_priority(realtime: bool) -> Result<()> {
    let error = unsafe { clef_audio_thread_priority(realtime.into()) };
    if error != 0 {
        return Err(
            format!("set audio thread scheduling policy failed (platform code {error})").into(),
        );
    }
    Ok(())
}

pub struct Activity(*mut std::ffi::c_void);
impl Activity {
    pub fn begin() -> Result<Self> {
        let token = unsafe { clef_audio_activity_begin() };
        if token.is_null() {
            return Err("could not begin realtime process activity".into());
        }
        Ok(Self(token))
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        unsafe {
            clef_audio_activity_end(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn realtime_activity_can_be_scoped_and_released() {
        let activity = super::Activity::begin().unwrap();
        drop(activity);
    }

    #[test]
    fn audio_thread_priority_can_be_set_and_released() {
        std::thread::spawn(|| {
            super::audio_priority(true).unwrap();
            assert_eq!(unsafe { super::clef_audio_thread_is_realtime() }, 1);
            super::audio_priority(false).unwrap();
            assert_eq!(unsafe { super::clef_audio_thread_is_realtime() }, 0);
        })
        .join()
        .unwrap();
    }
}
