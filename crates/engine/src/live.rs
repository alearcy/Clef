use crate::restart::InstrumentAudio;
use crate::{
    graph::Plan,
    protocol::{BLOCK, Frame, MAX_EVENTS, Midi, RATE, Result},
    rpc::{AudioConnection, Instrument},
    sequence,
};
pub use clef_audio_io::Configuration;
use clef_audio_io::{self as audio_io, MidiInputConnection, Stream, StreamTrait};
use rtrb::{Consumer, RingBuffer};
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::Instant,
};

const QUEUE_BLOCKS: usize = 4;
#[cfg(test)]
const LEAD: u64 = (QUEUE_BLOCKS * BLOCK) as u64;

struct Packet {
    generation: u64,
    at: u64,
    frames: [Frame; BLOCK],
}

#[derive(Default)]
pub struct Counters {
    rate: AtomicU32,
    buffer: AtomicU32,
    lead: AtomicU64,
    played: AtomicU64,
    generation: AtomicU64,
    reconfiguring: AtomicBool,
    reconfigurations: AtomicU64,
    transition_frames: AtomicU64,
    plugin_latency: AtomicU32,
    graph_latency: AtomicU32,
    callbacks: AtomicU64,
    min_callback: AtomicU64,
    max_callback: AtomicU64,
    misses: AtomicU64,
    errors: AtomicU64,
    stale: AtomicU64,
    midi_overflows: AtomicU64,
    late_midi: AtomicU64,
    midi_received: AtomicU64,
    panic: AtomicBool,
    failed: AtomicBool,
    peak: AtomicU32,
    max_rpc_us: AtomicU64,
    max_worker_us: AtomicU64,
    max_broker_interval_us: AtomicU64,
    max_queue_wait_us: AtomicU64,
}

impl Counters {
    pub fn report(&self) -> serde_json::Value {
        let read = |v: &AtomicU64| v.load(Ordering::Relaxed);
        serde_json::json!({
            "sample_rate": self.rate.load(Ordering::Relaxed), "requested_buffer_frames": self.buffer.load(Ordering::Relaxed),
            "transport_buffering_samples": read(&self.lead),
            "plugin_latency_samples": self.plugin_latency.load(Ordering::Acquire),
            "graph_latency_samples": self.graph_latency.load(Ordering::Acquire),
            "cumulative_latency_samples": read(&self.lead) + u64::from(self.plugin_latency.load(Ordering::Acquire)) + u64::from(self.graph_latency.load(Ordering::Acquire)),
            "reconfigurations": read(&self.reconfigurations),
            "transition_gap_frames": read(&self.transition_frames),
            "played_frames": read(&self.played), "callbacks": read(&self.callbacks),
            "min_callback_frames": if read(&self.callbacks) == 0 { 0 } else { read(&self.min_callback) },
            "max_callback_frames": read(&self.max_callback),
            "deadline_missed_blocks": read(&self.misses), "output_errors": read(&self.errors),
            "discarded_stale_blocks": read(&self.stale),
            "midi_overflows": read(&self.midi_overflows), "late_midi_events": read(&self.late_midi),
            "midi_received": read(&self.midi_received),
            "worker_failed": self.failed.load(Ordering::Acquire),
            "worker_peak": f32::from_bits(self.peak.load(Ordering::Relaxed)),
            "max_process_rpc_us": read(&self.max_rpc_us),
            "max_worker_process_us": read(&self.max_worker_us),
            "max_broker_interval_us": read(&self.max_broker_interval_us),
            "max_queue_wait_us": read(&self.max_queue_wait_us),
        })
    }
}

struct Output {
    queue: Consumer<Packet>,
    next: Option<Packet>,
    current: [Frame; BLOCK],
    cursor: usize,
    position: u64,
    generation: u64,
    queue_blocks: usize,
    lead: u64,
    channels: usize,
    pair: [usize; 2],
    counters: Arc<Counters>,
}

impl Output {
    fn fill(&mut self, output: &mut [f32]) {
        output.fill(0.0);
        let frames = output.len() / self.channels;
        self.counters.callbacks.fetch_add(1, Ordering::Relaxed);
        self.counters
            .min_callback
            .fetch_min(frames as u64, Ordering::Relaxed);
        self.counters
            .max_callback
            .fetch_max(frames as u64, Ordering::Relaxed);
        for frame in output.chunks_exact_mut(self.channels) {
            let generation = self.counters.generation.load(Ordering::Acquire);
            if generation != self.generation {
                self.generation = generation;
                self.current.fill([0.0; 2]);
                self.cursor = (self.position % BLOCK as u64) as usize;
            }
            if self.cursor == BLOCK {
                self.current.fill([0.0; 2]);
                // Bound work even when every queued block missed its deadline.
                for _ in 0..=self.queue_blocks {
                    if self.next.is_none() {
                        self.next = self.queue.pop().ok();
                    }
                    match self.next.as_ref() {
                        Some(packet)
                            if packet.generation != generation || packet.at < self.position =>
                        {
                            self.next = None;
                            self.counters.stale.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => break,
                    }
                }
                if self.next.as_ref().is_some_and(|p| p.at == self.position) {
                    self.current = self.next.take().expect("matched packet").frames;
                    self.counters.reconfiguring.store(false, Ordering::Release);
                } else if self.position >= self.lead
                    && !self.counters.reconfiguring.load(Ordering::Acquire)
                {
                    self.counters.misses.fetch_add(1, Ordering::Relaxed);
                }
                self.cursor = 0;
            }
            if self.counters.reconfiguring.load(Ordering::Acquire) {
                self.counters
                    .transition_frames
                    .fetch_add(1, Ordering::Relaxed);
            } else if !self.counters.failed.load(Ordering::Acquire) {
                frame[self.pair[0]] = self.current[self.cursor][0];
                frame[self.pair[1]] = self.current[self.cursor][1];
            }
            self.cursor += 1;
            self.position += 1;
        }
        self.counters.played.store(self.position, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
struct TimedMidi {
    at: u64,
    bytes: [u8; 3],
}

pub(crate) fn channel_message(bytes: &[u8]) -> Option<[u8; 3]> {
    let &status = bytes.first()?;
    if !(0x80..=0xef).contains(&status) {
        return None;
    }
    let size = if matches!(status & 0xf0, 0xc0 | 0xd0) {
        2
    } else {
        3
    };
    if bytes.len() != size || bytes[1..].iter().any(|&b| b > 127) {
        return None;
    }
    Some([status, bytes[1], *bytes.get(2).unwrap_or(&0)])
}

pub struct Live {
    stream: Stream,
    midi: Option<MidiInputConnection<()>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Option<AudioConnection>>>,
    pub counters: Arc<Counters>,
}

impl Live {
    pub fn start(
        instrument: &mut Instrument,
        device_index: usize,
        pair: [usize; 2],
        midi_index: Option<usize>,
        plan: Plan,
    ) -> Result<Self> {
        let device = clef_audio_io::output_by_index(device_index)?;
        Self::configured(
            instrument,
            Configuration {
                device,
                pair,
                midi_index,
                midi_id: None,
                rate: RATE,
                buffer: BLOCK as u32,
            },
            plan,
        )
    }

    pub fn configured(
        instrument: &mut Instrument,
        config: Configuration,
        mut plan: Plan,
    ) -> Result<Self> {
        let Configuration {
            device,
            pair,
            midi_index,
            midi_id,
            rate,
            buffer,
        } = config;
        let channel_count = audio_io::output_channels(&device, pair, rate, buffer)?
            .ok_or("selected device/pair does not support the requested F32 sample rate/buffer")?;
        if pair[0] == pair[1] {
            return Err("select two different output channels".into());
        }
        let channels = usize::from(channel_count);
        let queue_blocks = (buffer as usize).div_ceil(BLOCK).max(1) * QUEUE_BLOCKS;
        let lead = (queue_blocks * BLOCK) as u64;
        let counters = Arc::new(Counters {
            min_callback: AtomicU64::new(u64::MAX),
            rate: AtomicU32::new(rate),
            buffer: AtomicU32::new(buffer),
            lead: AtomicU64::new(lead),
            ..Default::default()
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (mut producer, consumer) = RingBuffer::new(queue_blocks);
        let (mut midi_tx, mut midi_rx) = RingBuffer::new(MAX_EVENTS);
        let origin = Arc::new(OnceLock::<Instant>::new());
        let midi_origin = Arc::clone(&origin);
        let physical_midi = midi_index.is_some() || midi_id.is_some();
        let midi = if physical_midi {
            let stats = Arc::clone(&counters);
            Some(audio_io::midi_input(
                "Clef T02 live input",
                "Clef T02",
                midi_id.as_deref(),
                midi_index,
                true,
                move |bytes| {
                    if let Some(bytes) = channel_message(bytes)
                        && let Some(origin) = midi_origin.get()
                    {
                        stats.midi_received.fetch_add(1, Ordering::Relaxed);
                        let at = (origin.elapsed().as_secs_f64() * f64::from(rate)) as u64;
                        if midi_tx.push(TimedMidi { at, bytes }).is_err() {
                            stats.midi_overflows.fetch_add(1, Ordering::Relaxed);
                            stats.panic.store(true, Ordering::Release);
                        }
                    }
                },
            )?)
        } else {
            None
        };
        let mut output = Output {
            queue: consumer,
            next: None,
            current: [[0.0; 2]; BLOCK],
            cursor: BLOCK,
            position: 0,
            generation: 0,
            queue_blocks,
            lead,
            channels,
            pair,
            counters: Arc::clone(&counters),
        };
        let errors = Arc::clone(&counters);
        let stream = audio_io::output_stream(
            &device,
            channel_count,
            rate,
            buffer,
            move |data| output.fill(data),
            move || {
                errors.errors.fetch_add(1, Ordering::Relaxed);
            },
        )?;
        instrument.start(false)?;
        let control = instrument.control_connection();
        let mut audio = InstrumentAudio::new(instrument.take_audio()?);
        let stats = Arc::clone(&counters);
        let stopping = Arc::clone(&stop);
        let initial_latency = instrument.latency;
        counters
            .plugin_latency
            .store(initial_latency, Ordering::Release);
        let graph_latency = plan.latency;
        counters
            .graph_latency
            .store(graph_latency, Ordering::Release);
        // The stream starts with LEAD silent samples; no plugin call runs in its callback.
        let thread = std::thread::spawn(move || {
            let mut elapsed = 0;
            let mut generation = 0;
            let mut latency = initial_latency;
            let mut events = Vec::with_capacity(MAX_EVENTS);
            let mut pending = None;
            let mut previous_block = Instant::now();
            let result: Result<()> = (|| {
                crate::threading::audio_priority(true)?;
                while !stopping.load(Ordering::Acquire) {
                    // A restart runs on a helper (T37 L2); the callback is silent meanwhile.
                    if let Some(restarted) = audio.poll() {
                        latency = restarted?;
                        plan.reset()?;
                        pending = None;
                        while midi_rx.pop().is_ok() {}
                        elapsed = stats.played.load(Ordering::Acquire).div_ceil(BLOCK as u64)
                            * BLOCK as u64;
                        stats.plugin_latency.store(latency, Ordering::Release);
                        stats.reconfigurations.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    if audio.ready().is_none() {
                        std::thread::sleep(crate::multitrack::idle_wait(rate));
                        continue;
                    }
                    if producer.slots() == 0 {
                        let waiting = Instant::now();
                        while producer.slots() == 0 && !stopping.load(Ordering::Acquire) {
                            std::thread::sleep(crate::multitrack::idle_wait(rate));
                        }
                        stats
                            .max_queue_wait_us
                            .fetch_max(waiting.elapsed().as_micros() as u64, Ordering::Relaxed);
                        if stopping.load(Ordering::Acquire) {
                            break;
                        }
                    }
                    let started = Instant::now();
                    stats.max_broker_interval_us.fetch_max(
                        started.duration_since(previous_block).as_micros() as u64,
                        Ordering::Relaxed,
                    );
                    previous_block = started;
                    if !physical_midi {
                        sequence::events(elapsed, BLOCK, true, &mut events);
                    } else {
                        events.clear();
                        if stats.panic.swap(false, Ordering::AcqRel) {
                            pending = None;
                            while midi_rx.pop().is_ok() {}
                            events.extend(sequence::panic_events());
                        }
                        while events.len() < MAX_EVENTS {
                            if pending.is_none() {
                                pending = midi_rx.pop().ok();
                            }
                            let Some(event) = pending else { break };
                            if event.at >= elapsed + BLOCK as u64 {
                                break;
                            }
                            if event.at < elapsed {
                                stats.late_midi.fetch_add(1, Ordering::Relaxed);
                            }
                            events.push(Midi {
                                offset: event.at.saturating_sub(elapsed) as u32,
                                bytes: event.bytes,
                            });
                            pending = None;
                        }
                        events.sort_by_key(|event| event.offset);
                    }
                    let rpc_started = Instant::now();
                    let Some(connection) = audio.ready() else {
                        continue;
                    };
                    let reply = connection.process(sequence::START + elapsed, &events, BLOCK)?;
                    stats
                        .max_rpc_us
                        .fetch_max(rpc_started.elapsed().as_micros() as u64, Ordering::Relaxed);
                    stats
                        .max_worker_us
                        .fetch_max(reply.processing_nanos / 1000, Ordering::Relaxed);
                    let peak = reply
                        .frames
                        .iter()
                        .flatten()
                        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
                    stats.peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
                    if reply.restart || reply.latency != latency {
                        stats.reconfiguring.store(true, Ordering::Release);
                        generation += 1;
                        stats.generation.store(generation, Ordering::Release);
                        connection.process(
                            sequence::START + elapsed,
                            &sequence::panic_events(),
                            BLOCK,
                        )?;
                        audio.begin_restart(&control);
                        continue;
                    }
                    let mut frames = [[0.0; 2]; BLOCK];
                    frames.copy_from_slice(plan.process(&reply.frames)?);
                    producer
                        .push(Packet {
                            generation,
                            at: elapsed + lead,
                            frames,
                        })
                        .map_err(|_| "audio queue unexpectedly full")?;
                    elapsed += BLOCK as u64;
                }
                Ok(())
            })();
            if let Err(error) = result {
                stats.failed.store(true, Ordering::Release);
                eprintln!(
                    "Live worker failed: {error}. Output becomes silent; use recreate explicitly."
                );
                return audio.into_inner();
            }
            if let Some(connection) = audio.ready() {
                let _ =
                    connection.process(sequence::START + elapsed, &sequence::panic_events(), BLOCK);
                let _ = connection.suspend();
            }
            audio.into_inner()
        });
        let mut live = Self {
            stream,
            midi,
            stop,
            thread: Some(thread),
            counters,
        };
        let _ = origin.set(Instant::now());
        if let Err(error) = live.stream.play() {
            live.finish(instrument)?;
            return Err(error.into());
        }
        println!(
            "Audio output: {}, channels {}–{}, {rate} Hz, requested buffer {buffer}, IPC lead {lead} samples, graph latency {} samples, plugin latency {initial_latency} samples.",
            audio_io::describe_output(&device)?,
            pair[0] + 1,
            pair[1] + 1,
            graph_latency
        );
        Ok(live)
    }

    fn finish(&mut self, instrument: &mut Instrument) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        self.midi.take();
        let _ = self.stream.pause();
        if let Some(thread) = self.thread.take()
            && let Some(audio) = thread.join().map_err(|_| "live broker panicked")?
        {
            instrument.return_audio(audio);
        }
        instrument.stop()
    }

    pub fn stop(mut self, instrument: &mut Instrument) -> Result<serde_json::Value> {
        self.finish(instrument)?;
        Ok(self.counters.report())
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.stream.pause();
        self.midi.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_discards_stale_audio_and_never_replays_a_late_block() {
        let (mut tx, rx) = RingBuffer::new(QUEUE_BLOCKS);
        tx.push(Packet {
            generation: 0,
            at: 0,
            frames: [[0.8; 2]; BLOCK],
        })
        .ok()
        .unwrap();
        tx.push(Packet {
            generation: 0,
            at: LEAD,
            frames: [[0.2, 0.3]; BLOCK],
        })
        .ok()
        .unwrap();
        let counters = Arc::new(Counters::default());
        let mut output = Output {
            queue: rx,
            next: None,
            current: [[0.0; 2]; BLOCK],
            cursor: BLOCK,
            position: LEAD,
            generation: 0,
            queue_blocks: QUEUE_BLOCKS,
            lead: LEAD,
            channels: 4,
            pair: [0, 1],
            counters: Arc::clone(&counters),
        };
        let mut frames = [9.0; 4 * BLOCK * 2];
        output.fill(&mut frames[..4 * 73]);
        output.fill(&mut frames[4 * 73..]);
        assert!(
            frames[..4 * BLOCK]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|f| *f == [0.2, 0.3, 0.0, 0.0])
        );
        assert!(frames[4 * BLOCK..].iter().all(|&f| f == 0.0));
        assert_eq!(counters.stale.load(Ordering::Relaxed), 1);
        assert_eq!(counters.misses.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn latency_transition_discards_old_generation_and_counts_silence() {
        let (mut tx, rx) = RingBuffer::new(QUEUE_BLOCKS);
        for (generation, at, sample) in [(0, LEAD, 0.9), (1, LEAD + BLOCK as u64, 0.2)] {
            tx.push(Packet {
                generation,
                at,
                frames: [[sample; 2]; BLOCK],
            })
            .ok()
            .unwrap();
        }
        let counters = Arc::new(Counters::default());
        counters.generation.store(1, Ordering::Release);
        counters.reconfiguring.store(true, Ordering::Release);
        let mut output = Output {
            queue: rx,
            next: None,
            current: [[0.8; 2]; BLOCK],
            cursor: BLOCK,
            position: LEAD,
            generation: 0,
            queue_blocks: QUEUE_BLOCKS,
            lead: LEAD,
            channels: 2,
            pair: [0, 1],
            counters: Arc::clone(&counters),
        };
        let mut frames = [0.0; BLOCK * 4];
        output.fill(&mut frames);
        assert!(frames[..BLOCK * 2].iter().all(|v| *v == 0.0));
        assert!(frames[BLOCK * 2..].iter().all(|v| *v == 0.2));
        assert_eq!(
            counters.transition_frames.load(Ordering::Acquire),
            BLOCK as u64
        );
        assert_eq!(counters.misses.load(Ordering::Acquire), 0);
        assert_eq!(counters.stale.load(Ordering::Acquire), 1);
    }

    #[test]
    fn failed_worker_outputs_silence_even_with_current_or_queued_audio() {
        let (mut tx, rx) = RingBuffer::new(QUEUE_BLOCKS);
        tx.push(Packet {
            generation: 0,
            at: LEAD,
            frames: [[0.7; 2]; BLOCK],
        })
        .ok()
        .unwrap();
        let counters = Arc::new(Counters::default());
        counters.failed.store(true, Ordering::Release);
        let mut output = Output {
            queue: rx,
            next: None,
            current: [[0.8; 2]; BLOCK],
            cursor: 0,
            position: LEAD,
            generation: 0,
            queue_blocks: QUEUE_BLOCKS,
            lead: LEAD,
            channels: 2,
            pair: [0, 1],
            counters,
        };
        let mut frames = [1.0; BLOCK * 4];
        output.fill(&mut frames);
        assert!(frames.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn physical_midi_preserves_channels_and_rejects_system_or_malformed_messages() {
        assert_eq!(channel_message(&[0x9f, 60, 100]), Some([0x9f, 60, 100]));
        assert_eq!(channel_message(&[0xc4, 7]), Some([0xc4, 7, 0]));
        assert_eq!(channel_message(&[0xe2, 0, 64]), Some([0xe2, 0, 64]));
        for bad in [&[][..], &[0xf8], &[0x90, 60], &[0x90, 128, 0]] {
            assert_eq!(channel_message(bad), None);
        }
    }
}
