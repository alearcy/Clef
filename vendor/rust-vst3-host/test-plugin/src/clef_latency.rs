//! Clef-only build variant: real 0/257-sample latency and restart notifications.
use super::*;
use std::thread::{self, ThreadId};

const PARAM: u32 = 9000;
pub(super) const EFFECT: bool = cfg!(feature = "clef-insert-fixture");
struct Delay {
    samples: [[f32; 2]; 257],
    cursor: usize,
    elapsed: u64,
}
pub(super) struct Fixture {
    main: ThreadId,
    requested: AtomicU32,
    applied: AtomicU32,
    active: AtomicBool,
    realtime: AtomicBool,
    notified: AtomicBool,
    processing: AtomicBool,
    handler: Mutex<Option<vst3::ComPtr<IComponentHandler>>>,
    delay: Mutex<Delay>,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            main: thread::current().id(),
            requested: AtomicU32::new(0),
            applied: AtomicU32::new(0),
            active: AtomicBool::new(false),
            realtime: AtomicBool::new(true),
            notified: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            handler: Mutex::new(None),
            delay: Mutex::new(Delay {
                samples: [[0.0; 2]; 257],
                cursor: 0,
                elapsed: 0,
            }),
        }
    }
    pub fn latency(&self) -> u32 {
        if thread::current().id() != self.main || self.active.load(Ordering::Acquire) {
            u32::MAX
        } else {
            self.applied.load(Ordering::Acquire)
        }
    }
    pub fn setup(&self, realtime: bool) -> tresult {
        if thread::current().id() != self.main || self.active.load(Ordering::Acquire) {
            return kInvalidArgument;
        }
        self.realtime.store(realtime, Ordering::Release);
        self.applied
            .store(self.requested.load(Ordering::Acquire), Ordering::Release);
        *self.delay.lock().unwrap() = Delay {
            samples: [[0.0; 2]; 257],
            cursor: 0,
            elapsed: 0,
        };
        kResultOk
    }
    pub fn activate(&self, active: bool) -> tresult {
        if thread::current().id() != self.main || self.processing.load(Ordering::Acquire) {
            return kInvalidArgument;
        }
        self.active.store(active, Ordering::Release);
        kResultOk
    }
    pub fn processing(&self, processing: bool) -> tresult {
        if thread::current().id() == self.main || !self.active.load(Ordering::Acquire) {
            return kInvalidArgument;
        }
        self.processing.store(processing, Ordering::Release);
        kResultOk
    }
    pub unsafe fn handler(&self, handler: *mut IComponentHandler) {
        *self.handler.lock().unwrap() = ComRef::from_raw(handler).map(|h| h.to_com_ptr());
    }
    pub fn value(&self, id: u32) -> f64 {
        if id == PARAM {
            f64::from(self.requested.load(Ordering::Acquire)) / 257.0
        } else {
            0.0
        }
    }
    pub unsafe fn set(&self, id: u32, value: f64) -> tresult {
        if id != PARAM
            || !value.is_finite()
            || !(0.0..=1.0).contains(&value)
            || thread::current().id() != self.main
        {
            return kInvalidArgument;
        }
        self.requested
            .store(if value < 0.5 { 0 } else { 257 }, Ordering::Release);
        if let Some(handler) = self.handler.lock().unwrap().as_ref() {
            return handler.restartComponent(RestartFlags_::kLatencyChanged as i32);
        }
        kResultOk
    }
    pub unsafe fn parameter_info(index: i32, info: *mut ParameterInfo) -> tresult {
        if index != 0 || info.is_null() {
            return kInvalidArgument;
        }
        *info = std::mem::zeroed();
        (*info).id = PARAM;
        (*info).stepCount = 1;
        (*info).flags = ParameterInfo_::ParameterFlags_::kCanAutomate as i32;
        copy_wstring("Latency 0 or 257", &mut (*info).title);
        kResultOk
    }
    pub unsafe fn save(&self, stream: *mut IBStream) -> tresult {
        if stream_write_all(
            stream,
            &self.requested.load(Ordering::Acquire).to_le_bytes(),
        ) {
            kResultOk
        } else {
            kResultFalse
        }
    }
    pub unsafe fn restore(&self, stream: *mut IBStream) -> tresult {
        let Some(bytes) = stream_read_exact(stream, 4) else {
            return kResultFalse;
        };
        let value = u32::from_le_bytes(bytes.try_into().unwrap());
        if ![0, 257].contains(&value) || self.active.load(Ordering::Acquire) {
            return kInvalidArgument;
        }
        self.requested.store(value, Ordering::Release);
        kResultOk
    }
    pub unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        if data.is_null()
            || thread::current().id() == self.main
            || !self.processing.load(Ordering::Acquire)
        {
            return kInvalidArgument;
        }
        let data = &*data;
        if !(0..=128).contains(&data.numSamples) || data.numOutputs != 1 || data.outputs.is_null() {
            return kInvalidArgument;
        }
        let output = &*data.outputs;
        if output.numChannels != 2 || output.__field0.channelBuffers32.is_null() {
            return kInvalidArgument;
        }
        let left = *output.__field0.channelBuffers32;
        let right = *output.__field0.channelBuffers32.add(1);
        if left.is_null() || right.is_null() {
            return kInvalidArgument;
        }
        let mut input = [[0.0_f32; 2]; 128];
        if EFFECT {
            if data.numInputs != 1 || data.inputs.is_null() {
                return kInvalidArgument;
            }
            let bus = &*data.inputs;
            let channels = bus.__field0.channelBuffers32;
            if bus.numChannels != 2 || channels.is_null() {
                return kInvalidArgument;
            }
            for channel in 0..2 {
                let source = *channels.add(channel);
                if source.is_null() {
                    return kInvalidArgument;
                }
                for (index, frame) in input[..data.numSamples as usize].iter_mut().enumerate() {
                    frame[channel] = *source.add(index) * 0.5;
                }
            }
        } else if let Some(events) = ComRef::from_raw(data.inputEvents) {
            let count = events.getEventCount();
            if !(0..=2048).contains(&count) {
                return kInvalidArgument;
            }
            for i in 0..count {
                let mut event: Event = std::mem::zeroed();
                if events.getEvent(i, &mut event) != kResultOk {
                    return kInvalidArgument;
                }
                if event.r#type as u32 == Event_::EventTypes_::kNoteOnEvent as u32
                    && event.__field0.noteOn.velocity > 0.0
                {
                    if event.sampleOffset < 0 || event.sampleOffset >= data.numSamples {
                        return kInvalidArgument;
                    }
                    input[event.sampleOffset as usize] = [0.25; 2];
                }
            }
        }
        let latency = self.applied.load(Ordering::Acquire) as usize;
        let mut delay = self.delay.lock().unwrap();
        for (i, &sample) in input[..data.numSamples as usize].iter().enumerate() {
            let value = if latency == 0 {
                sample
            } else {
                let cursor = delay.cursor;
                let value = delay.samples[cursor];
                delay.samples[cursor] = sample;
                delay.cursor = (cursor + 1) % latency;
                value
            };
            *left.add(i) = value[0];
            *right.add(i) = value[1];
        }
        delay.elapsed += data.numSamples as u64;
        if delay.elapsed >= 48_000
            && self.realtime.load(Ordering::Acquire)
            && !self.notified.swap(true, Ordering::AcqRel)
        {
            let Some(changes) = ComRef::from_raw(data.outputParameterChanges) else {
                return kInvalidArgument;
            };
            let mut index = 0;
            let Some(queue) = ComRef::from_raw(changes.addParameterData(&PARAM, &mut index)) else {
                return kInvalidArgument;
            };
            if queue.addPoint(0, 1.0, &mut index) != kResultOk {
                return kResultFalse;
            }
        }
        kResultOk
    }
}
