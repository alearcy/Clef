use crate::{
    graph::Plan,
    protocol::{BLOCK, MAX_EVENTS, Result},
    rpc::Instrument,
    sequence,
    wav::Wav,
};
use std::path::Path;

unsafe extern "C" {
    fn clef_render_cancel_begin() -> i32;
    fn clef_render_cancelled() -> i32;
    fn clef_render_cancel_end();
}

struct Cancellation;
impl Cancellation {
    fn check(&self) -> Result<()> {
        if unsafe { clef_render_cancelled() } != 0 {
            return Err("render cancelled; no completed file was published".into());
        }
        Ok(())
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        unsafe {
            clef_render_cancel_end();
        }
    }
}

pub fn render(instrument: &mut Instrument, path: &Path, mut plan: Plan) -> Result<u64> {
    if unsafe { clef_render_cancel_begin() } != 0 {
        return Err("could not install render cancellation handler".into());
    }
    let cancellation = Cancellation;
    let expected = sequence::LENGTH + sequence::TAIL;
    let mut wav = Wav::create(path, expected)?;
    let result: Result<()> = (|| {
        instrument.start(true)?;
        let plugin_latency = instrument.latency;
        let latency = u64::from(plugin_latency) + u64::from(plan.latency);
        let total = expected + latency;
        let mut events = Vec::with_capacity(MAX_EVENTS);
        let mut elapsed = 0;
        while elapsed < total {
            cancellation.check()?;
            let frames = BLOCK.min((total - elapsed) as usize);
            sequence::events(elapsed, frames, false, &mut events);
            let (audio, restart) =
                instrument.process(sequence::START + elapsed, &events, frames)?;
            if restart || instrument.latency != plugin_latency {
                return Err("plugin requested reconfiguration during offline render".into());
            }
            let audio = plan.process(&audio)?;
            let skip = latency.saturating_sub(elapsed).min(frames as u64) as usize;
            wav.write(&audio[skip..])?;
            elapsed += frames as u64;
        }
        Ok(())
    })();
    let stopped = instrument.stop();
    cancellation.check()?;
    result?;
    stopped?;
    wav.finish(expected)?;
    Ok(expected)
}
