#include <mach/mach.h>
#include <mach/mach_time.h>
#include <mach/thread_policy.h>
#include "public.sdk/source/vst/hosting/module.h"
#include "public.sdk/source/vst/hosting/connectionproxy.h"
#include "public.sdk/source/vst/hosting/hostclasses.h"
#include "public.sdk/source/vst/hosting/eventlist.h"
#include "public.sdk/source/vst/hosting/parameterchanges.h"
#include "public.sdk/source/common/memorystream.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivstprocesscontext.h"
#include "pluginterfaces/vst/ivstattributes.h"
#include "pluginterfaces/vst/vstpresetkeys.h"
#include "pluginterfaces/gui/iplugview.h"
#include "pluginterfaces/base/funknownimpl.h"
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>
#include <pthread/qos.h>
#include <vector>

using namespace Steinberg;
using namespace Steinberg::Vst;
namespace {
constexpr size_t block = 128, max_events = 1024, max_state = 64 * 1024 * 1024;
thread_local char error_text[1024] = {};
void require(bool ok, const char* message) { if (!ok) throw std::runtime_error(message); }
bool success(tresult result) { return result == kResultOk || result == kResultTrue; }
template<class F> int guarded(F&& f) noexcept {
    try { f(); return 1; }
    catch (const std::exception& e) { std::snprintf(error_text, sizeof(error_text), "%s", e.what()); }
    catch (...) { std::snprintf(error_text, sizeof(error_text), "unknown native VST3 exception"); }
    return 0;
}

template<class T, size_t N> struct Queue {
    std::array<T, N> values {};
    alignas(64) std::atomic<size_t> read {0};
    alignas(64) std::atomic<size_t> write {0};
    bool push(T value) {
        auto w = write.load(std::memory_order_relaxed);
        auto next = (w + 1) % N;
        if (next == read.load(std::memory_order_acquire)) return false;
        values[w] = value;
        write.store(next, std::memory_order_release);
        return true;
    }
    bool pop(T& value) {
        auto r = read.load(std::memory_order_relaxed);
        if (r == write.load(std::memory_order_acquire)) return false;
        value = values[r];
        read.store((r + 1) % N, std::memory_order_release);
        return true;
    }
    void clear_stopped() { read.store(write.load(std::memory_order_acquire), std::memory_order_release); }
};
struct Edit { ParamID id; double value; };
struct Signals {
    std::thread::id main = std::this_thread::get_id();
    Queue<Edit, 4096> edits, feedback;
    std::atomic<uint32_t> restart {0};
    std::atomic<uint64_t> resize {0};
    std::atomic<bool> overflow {false};
};

class Handler final : public U::Implements<U::Directly<IComponentHandler, IPlugFrame>> {
    Signals& signals;
public:
    explicit Handler(Signals& signals) : signals(signals) {}
    tresult PLUGIN_API beginEdit(ParamID) override { return kResultOk; }
    tresult PLUGIN_API endEdit(ParamID) override { return kResultOk; }
    tresult PLUGIN_API performEdit(ParamID id, ParamValue value) override {
        if (std::this_thread::get_id() != signals.main || !std::isfinite(value) || value < 0 || value > 1)
            return kInvalidArgument;
        if (!signals.edits.push({id, value})) {
            signals.overflow.store(true, std::memory_order_release);
            return kOutOfMemory;
        }
        return kResultOk;
    }
    tresult PLUGIN_API restartComponent(int32 flags) override {
        signals.restart.fetch_or(static_cast<uint32_t>(flags), std::memory_order_release);
        return kResultOk;
    }
    tresult PLUGIN_API resizeView(IPlugView*, ViewRect* rect) override {
        if (!rect) return kInvalidArgument;
        int64 w = int64(rect->right) - rect->left, h = int64(rect->bottom) - rect->top;
        if (w <= 0 || h <= 0 || w > 8192 || h > 8192) return kInvalidArgument;
        signals.resize.store((uint64_t(w) << 32) | uint32_t(h), std::memory_order_release);
        return kResultOk;
    }
};

class Host final : public HostApplication {
    tresult PLUGIN_API getName(String128 name) override {
        std::fill_n(name, 128, char16(0));
        for (size_t i = 0; i < 4; ++i) name[i] = u"Clef"[i];
        return kResultOk;
    }
};

// SDK MemoryStream supplies storage/COM; cap writes and seeks at the ABI boundary.
class StateStream : public MemoryStream, public IStreamAttributes {
    bool read_only = false;
    IPtr<IAttributeList> attributes = HostAttributeList::make();
    void metadata() { attributes->setString(PresetAttributes::kStateType, u"Project"); }
public:
    StateStream() { metadata(); }
    StateStream(const uint8_t* data, size_t size) : MemoryStream(const_cast<uint8_t*>(data), size), read_only(true) { metadata(); }
    uint32 PLUGIN_API addRef() override { return MemoryStream::addRef(); }
    uint32 PLUGIN_API release() override { return MemoryStream::release(); }
    tresult PLUGIN_API queryInterface(const TUID iid, void** object) override {
        if (FUnknownPrivate::iidEqual(iid, IStreamAttributes::iid)) {
            *object = static_cast<IStreamAttributes*>(this); addRef(); return kResultOk;
        }
        return MemoryStream::queryInterface(iid, object);
    }
    tresult PLUGIN_API getFileName(String128) override { return kNotImplemented; }
    IAttributeList* PLUGIN_API getAttributes() override { return attributes; }
    tresult PLUGIN_API write(void* data, int32 count, int32* written) override {
        if (read_only) return kNotImplemented;
        if (count < 0 || cursor < 0 || uint64_t(cursor) + uint32_t(count) > max_state) {
            if (written) *written = 0;
            return kOutOfMemory;
        }
        return MemoryStream::write(data, count, written);
    }
    tresult PLUGIN_API seek(int64 offset, int32 mode, int64* result) override {
        int64 base = mode == IBStream::kIBSeekSet ? 0 : mode == IBStream::kIBSeekCur ? cursor : size;
        if (mode < 0 || mode > IBStream::kIBSeekEnd || offset < -base || offset > int64(max_state) - base)
            return kInvalidArgument;
        return MemoryStream::seek(offset, mode, result);
    }
    tresult PLUGIN_API read(void* data, int32 count, int32* read) override {
        if (count < 0) return kInvalidArgument;
        return MemoryStream::read(data, count, read);
    }
};

struct Midi { uint32_t offset; uint8_t bytes[3]; };
struct Bridge {
    // Module must outlive every plugin interface and the provider's termination calls.
    VST3::Hosting::Module::Ptr module;
    IPtr<Host> host = owned(new Host);
    Signals signals;
    IPtr<Handler> handler = owned(new Handler(signals));
    IPtr<ConnectionProxy> component_connection, controller_connection;
    bool component_initialized = false, controller_initialized = false;
    IPtr<IComponent> component;
    IPtr<IEditController> controller;
    IPtr<IAudioProcessor> processor;
    IPtr<IPlugView> view;
    bool single_component = false, active = false;
    std::atomic<bool> processing {false};
    std::thread::id audio_thread;
    uint32_t latency = 0;
    int mode = kRealtime;
    double sample_rate = 48000.0;
    std::string name;
    std::array<std::array<ParamID, 131>, 16> mapping;
    EventList events {int32(max_events * 2)};
    std::array<uint16_t, 16 * 128> held_notes {};
    size_t held_count = 0;
    ParameterChanges input_params {4096}, output_params {4096};
    ProcessContext context {};
    ProcessData data {};
    std::array<std::array<float, block>, 2> channels {};
    std::array<float*, 2> pointers {channels[0].data(), channels[1].data()};
    AudioBusBuffers output {};
    std::vector<uint8_t> snapshot;

    void main_thread() const { require(std::this_thread::get_id() == signals.main, "VST3 main-thread operation on wrong thread"); }
    void close() {
        main_thread();
        if (view) {
            auto removed = view->removed();
            auto cleared = view->setFrame(nullptr);
            view = nullptr;
            signals.resize.store(0, std::memory_order_release);
            require(success(removed) && success(cleared), "VST3 editor detach failed");
        }
        signals.resize.store(0, std::memory_order_release);
    }
    ~Bridge() {
        try { close(); } catch (...) {}
        if (processing.load()) std::abort();
        if (component && active) component->setActive(false);
        if (controller) controller->setComponentHandler(nullptr);
        if (component_connection) component_connection->disconnect();
        if (controller_connection) controller_connection->disconnect();
        component_connection = nullptr; controller_connection = nullptr;
        if (controller_initialized && controller) controller->terminate();
        if (component_initialized && component) component->terminate();
        view = nullptr; processor = nullptr; controller = nullptr; component = nullptr;
    }
    void load(const char* path, const char* selected) {
        std::string error;
        module = VST3::Hosting::Module::create(path, error);
        require(bool(module), error.c_str());
        auto& factory = module->getFactory();
        factory.setHostContext(host);
        for (auto& info : factory.classInfos()) {
            if (info.category() != kVstAudioEffectClass) continue;
            if (selected && *selected && info.ID().toString() != selected) continue;
            name = info.name();
            component = factory.createInstance<IComponent>(info.ID());
            require(bool(component), "VST3 component creation failed");
            require(success(component->initialize(host)), "VST3 component initialization failed");
            component_initialized = true;
            FUnknownPtr<IEditController> combined(component);
            single_component = bool(combined);
            if (single_component) controller = combined;
            else {
                TUID id;
                require(success(component->getControllerClassId(id)), "VST3 controller class unavailable");
                controller = factory.createInstance<IEditController>(VST3::UID(id));
                require(bool(controller), "VST3 controller creation failed");
                require(success(controller->initialize(host)), "VST3 controller initialization failed");
                controller_initialized = true;
            }
            break;
        }
        require(component && controller, "T02 requires a VST3 component and controller");
        FUnknownPtr<IAudioProcessor> proc(component); processor = proc;
        require(bool(processor), "VST3 component has no audio processor");
        require(success(controller->setComponentHandler(handler)), "VST3 rejected component handler");
        if (!single_component) {
            FUnknownPtr<IConnectionPoint> comp(component), ctrl(controller);
            // IConnectionPoint is optional; the SDK provider assumes both sides expose it.
            if (comp && ctrl) {
                component_connection = owned(new ConnectionProxy(comp));
                controller_connection = owned(new ConnectionProxy(ctrl));
                require(success(component_connection->connect(ctrl)) && success(controller_connection->connect(comp)),
                    "VST3 component/controller connection failed");
            }
        }
        require(component->getBusCount(kAudio, kInput) == 0 && component->getBusCount(kAudio, kOutput) == 1,
            "T02 bridge requires a zero-input single-stereo-output VST3 instrument");
        BusInfo info {};
        require(success(component->getBusInfo(kAudio, kOutput, 0, info)) && info.channelCount == 2, "VST3 output is not stereo");
        SpeakerArrangement stereo = SpeakerArr::kStereo;
        require(success(processor->setBusArrangements(nullptr, 0, &stereo, 1)), "VST3 rejected stereo arrangement");
        require(success(component->activateBus(kAudio, kOutput, 0, true)), "VST3 output activation failed");
        int32 note_buses = component->getBusCount(kEvent, kInput);
        require(note_buses > 0 && note_buses <= 32, "VST3 has no usable event input");
        for (int32 i = 0; i < note_buses; ++i)
            require(success(component->activateBus(kEvent, kInput, i, i == 0)), "VST3 note-bus activation failed");
        require(success(processor->canProcessSampleSize(kSample32)), "VST3 does not support float32 processing");
        FUnknownPtr<IMidiMapping> midi(controller);
        for (int channel = 0; channel < 16; ++channel) {
            mapping[channel].fill(kNoParamId);
            if (midi) for (int cc = 0; cc < 131; ++cc) {
                ParamID id;
                if (success(midi->getMidiControllerAssignment(0, channel, int16(cc), id))) mapping[channel][cc] = id;
            }
        }
        output.numChannels = 2; output.channelBuffers32 = pointers.data();
        data.symbolicSampleSize = kSample32; data.numOutputs = 1; data.outputs = &output;
        data.inputEvents = &events; data.inputParameterChanges = &input_params;
        data.outputParameterChanges = &output_params; data.processContext = &context;
    }
    void activate(bool offline, double rate) {
        require(rate >= 8000 && rate <= 384000, "invalid sample rate");
        sample_rate = rate;
        main_thread(); require(!processing.load(std::memory_order_acquire), "suspend VST3 audio before activation");
        if (active) { require(success(component->setActive(false)), "VST3 deactivation failed"); active = false; }
        mode = offline ? kOffline : kRealtime;
        ProcessSetup setup {mode, kSample32, int32(block), sample_rate};
        require(success(processor->setupProcessing(setup)), "VST3 setupProcessing failed");
        latency = processor->getLatencySamples();
        if (latency > 480000) throw std::runtime_error("VST3 latency exceeds prototype capacity: " + std::to_string(latency));
        require(success(component->setActive(true)), "VST3 activation failed"); active = true;
        signals.restart.store(0, std::memory_order_release);
    }
    void release_notes(int channel, int32 offset) {
        for (int pitch = 0; pitch < 128; ++pitch) {
            auto& count = held_notes[size_t(channel) * 128 + pitch];
            while (count > 0) {
                Event event {}; event.type = Event::kNoteOffEvent; event.sampleOffset = offset;
                event.noteOff.channel = int16(channel); event.noteOff.pitch = int16(pitch); event.noteOff.noteId = -1;
                require(success(events.addEvent(event)), "VST3 note cleanup overflow");
                --count; --held_count;
            }
        }
    }
    void stop_audio() {
        if (!processing.load(std::memory_order_acquire)) return;
        require(std::this_thread::get_id() == audio_thread, "VST3 stopProcessing on wrong thread");
        events.clear(); input_params.clearQueue(); output_params.clearQueue();
        for (int channel = 0; channel < 16; ++channel) {
            release_notes(channel, 0);
            parameter(mapping[channel][64], 0, 0);
        }
        data.numSamples = int32(block);
        for (auto& channel : channels) channel.fill(0);
        require(success(processor->process(data)), "VST3 note cleanup process failed");
        auto result = processor->setProcessing(false);
        require(success(result) || result == kNotImplemented, "VST3 stopProcessing failed");
        processing.store(false, std::memory_order_release);
    }
    void parameter(ParamID id, double value, int32 offset) {
        if (id == kNoParamId) return;
        int32 index;
        auto* queue = input_params.addParameterData(id, index);
        require(queue && success(queue->addPoint(offset, value, index)), "VST3 parameter queue failed");
    }
    void process(uint64_t position, const Midi* midi, size_t count, size_t frames, float* interleaved) {
        require(active && frames > 0 && frames <= block && count <= max_events, "invalid VST3 process block");
        if (!processing.load(std::memory_order_acquire)) {
            audio_thread = std::this_thread::get_id();
            auto result = processor->setProcessing(true);
            require(success(result) || result == kNotImplemented, "VST3 startProcessing failed");
            processing.store(true, std::memory_order_release);
        }
        require(std::this_thread::get_id() == audio_thread, "VST3 process changed thread");
        events.clear(); input_params.clearQueue(); output_params.clearQueue();
        Edit edit;
        while (signals.edits.pop(edit)) parameter(edit.id, edit.value, 0);
        for (size_t i = 0; i < count; ++i) {
            auto& m = midi[i]; auto status = m.bytes[0] & 0xf0; auto ch = m.bytes[0] & 15;
            require(m.offset < frames && m.bytes[0] >= 0x80 && m.bytes[0] <= 0xef && m.bytes[1] < 128 && m.bytes[2] < 128, "invalid MIDI event");
            if (status == 0xb0) {
                if (m.bytes[1] == 120 || m.bytes[1] == 123) release_notes(ch, int32(m.offset));
                parameter(mapping[ch][m.bytes[1]], m.bytes[2] / 127.0, m.offset); continue;
            }
            if (status == 0xe0) { parameter(mapping[ch][129], (m.bytes[1] | (m.bytes[2] << 7)) / 16383.0, m.offset); continue; }
            if (status == 0xd0) { parameter(mapping[ch][128], m.bytes[1] / 127.0, m.offset); continue; }
            if (status == 0xc0) { parameter(mapping[ch][130], m.bytes[1] / 127.0, m.offset); continue; }
            Event event {}; event.busIndex = 0; event.sampleOffset = int32(m.offset);
            if (status == 0x90 && m.bytes[2] > 0) {
                event.type = Event::kNoteOnEvent;
                event.noteOn.channel = int16(ch); event.noteOn.pitch = m.bytes[1];
                event.noteOn.velocity = m.bytes[2] / 127.f; event.noteOn.noteId = -1;
                require(held_count < max_events, "VST3 held-note tracking capacity exceeded");
                ++held_notes[size_t(ch) * 128 + m.bytes[1]]; ++held_count;
            } else if (status == 0xa0) {
                event.type = Event::kPolyPressureEvent;
                event.polyPressure.channel = int16(ch); event.polyPressure.pitch = m.bytes[1];
                event.polyPressure.pressure = m.bytes[2] / 127.f; event.polyPressure.noteId = -1;
            } else {
                event.type = Event::kNoteOffEvent;
                event.noteOff.channel = int16(ch); event.noteOff.pitch = m.bytes[1];
                event.noteOff.velocity = status == 0x80 ? m.bytes[2] / 127.f : 0; event.noteOff.noteId = -1;
                auto& count = held_notes[size_t(ch) * 128 + m.bytes[1]];
                if (count > 0) { --count; --held_count; }
            }
            require(success(events.addEvent(event)), "VST3 event queue overflow");
        }
        context = {}; context.sampleRate = sample_rate; context.projectTimeSamples = int64(position);
        context.continousTimeSamples = int64(position); context.projectTimeMusic = position / 28800.0;
        context.tempo = 100; context.timeSigNumerator = 4; context.timeSigDenominator = 4;
        context.barPositionMusic = std::floor(context.projectTimeMusic / 4) * 4;
        context.state = ProcessContext::kPlaying | ProcessContext::kProjectTimeMusicValid
            | ProcessContext::kTempoValid | ProcessContext::kTimeSigValid | ProcessContext::kBarPositionValid | ProcessContext::kContTimeValid;
        data.processMode = mode; data.numSamples = int32(frames); output.silenceFlags = 0;
        for (auto& channel : channels) channel.fill(0);
        require(success(processor->process(data)), "VST3 process failed");
        for (int32 i = 0; i < output_params.getParameterCount(); ++i) {
            auto* queue = output_params.getParameterData(i); int32 offset; double value;
            auto n = queue->getPointCount();
            if (n > 0 && success(queue->getPoint(n-1, offset, value)))
                if (!signals.feedback.push({queue->getParameterId(), value})) signals.overflow.store(true, std::memory_order_release);
        }
        for (size_t i = 0; i < frames; ++i) for (size_t ch = 0; ch < 2; ++ch) {
            require(std::isfinite(channels[ch][i]), "VST3 produced non-finite audio");
            interleaved[i * 2 + ch] = channels[ch][i];
        }
    }
    void resize(int32& w, int32& h) {
        require(view && w > 0 && h > 0 && w <= 8192 && h <= 8192, "invalid VST3 editor resize");
        ViewRect rect {0, 0, w, h};
        if (view->canResize() == kResultTrue) {
            auto result = view->checkSizeConstraint(&rect);
            require(success(result) || result == kResultFalse || result == kNotImplemented, "VST3 size constraint failed");
            require(rect.right > rect.left && rect.bottom > rect.top && int64(rect.right)-rect.left <= 8192 && int64(rect.bottom)-rect.top <= 8192, "VST3 editor size exceeds capacity");
            require(success(view->onSize(&rect)), "VST3 editor rejected resize");
        } else require(success(view->getSize(&rect)), "VST3 editor size unavailable");
        w = rect.right - rect.left; h = rect.bottom - rect.top;
    }
    void save() {
        main_thread(); require(!active, "deactivate VST3 before saving state");
        StateStream comp, ctrl;
        require(success(component->getState(&comp)), "VST3 component getState failed");
        auto result = single_component ? kNotImplemented : controller->getState(&ctrl);
        require(success(result) || result == kNotImplemented || result == kResultFalse, "VST3 controller getState failed");
        uint32_t cl = uint32_t(comp.getSize()), tl = success(result) ? uint32_t(ctrl.getSize()) : UINT32_MAX;
        size_t total = 28 + size_t(cl) + (tl == UINT32_MAX ? 0 : tl);
        require(total <= max_state, "combined VST3 state exceeds limit");
        snapshot.resize(total); std::memcpy(snapshot.data(), "VST3HOST_STATE\0\0", 16);
        auto put = [&](size_t at, uint32_t value) { for (size_t i = 0; i < 4; ++i) snapshot[at+i] = uint8_t(value >> (8*i)); };
        put(16, 1); put(20, cl); put(24, tl);
        if (cl) std::memcpy(snapshot.data()+28, comp.getData(), cl);
        if (tl != UINT32_MAX && tl) std::memcpy(snapshot.data()+28+cl, ctrl.getData(), tl);
    }
    void restore(const uint8_t* bytes, size_t size) {
        main_thread(); require(!active && size <= max_state, "invalid VST3 restore lifecycle or size");
        const uint8_t* component_bytes = bytes; size_t component_size = size;
        const uint8_t* controller_bytes = nullptr; size_t controller_size = 0;
        if (size >= 16 && std::memcmp(bytes, "VST3HOST_STATE\0\0", 16) == 0) {
            require(size >= 28, "truncated VST3 state header");
            auto get = [&](size_t at) { uint32_t value = 0; for (size_t i=0;i<4;++i) value |= uint32_t(bytes[at+i]) << (8*i); return value; };
            require(get(16) == 1, "unsupported VST3 state version");
            component_size = get(20); auto ctrl_size = get(24);
            controller_size = ctrl_size == UINT32_MAX ? 0 : ctrl_size;
            require(uint64_t(28) + component_size + controller_size == size, "invalid VST3 state lengths");
            component_bytes = bytes + 28;
            if (ctrl_size != UINT32_MAX) controller_bytes = component_bytes + component_size;
        }
        signals.edits.clear_stopped(); signals.feedback.clear_stopped();
        StateStream comp(component_bytes, component_size);
        require(success(component->setState(&comp)), "VST3 component setState failed");
        if (!single_component) {
            StateStream sync(component_bytes, component_size);
            auto result = controller->setComponentState(&sync);
            require(success(result) || result == kNotImplemented, "VST3 controller setComponentState failed");
        }
        if (controller_bytes && !single_component) {
            StateStream ctrl(controller_bytes, controller_size);
            require(success(controller->setState(&ctrl)), "VST3 controller setState failed");
        }
    }
};
Bridge& bridge(void* pointer) { require(pointer != nullptr, "null VST3 bridge handle"); return *static_cast<Bridge*>(pointer); }
}

extern "C" {
int clef_audio_thread_priority(int realtime) noexcept {
    auto thread = pthread_mach_thread_np(pthread_self());
    if (!realtime) {
        return thread_policy_set(thread, THREAD_STANDARD_POLICY, nullptr, 0);
    }
    mach_timebase_info_data_t timebase {};
    auto result = mach_timebase_info(&timebase);
    if (result != KERN_SUCCESS) return result;
    auto period = uint32_t((1'000'000'000ULL * block / 48000) * timebase.denom / timebase.numer);
    thread_time_constraint_policy_data_t policy {period, period / 2, period, true};
    return thread_policy_set(thread, THREAD_TIME_CONSTRAINT_POLICY,
        reinterpret_cast<thread_policy_t>(&policy), THREAD_TIME_CONSTRAINT_POLICY_COUNT);
}
int clef_audio_thread_is_realtime() noexcept {
    thread_time_constraint_policy_data_t policy {};
    mach_msg_type_number_t count = THREAD_TIME_CONSTRAINT_POLICY_COUNT;
    boolean_t is_default = false;
    auto result = thread_policy_get(pthread_mach_thread_np(pthread_self()), THREAD_TIME_CONSTRAINT_POLICY,
        reinterpret_cast<thread_policy_t>(&policy), &count, &is_default);
    return result == KERN_SUCCESS && !is_default;
}
const char* clef_vst_error() noexcept { return error_text; }
int clef_vst_load(const char* path, const char* selected, void** out) noexcept { return guarded([&] {
    require(path && out, "null VST3 load argument"); auto instance = std::make_unique<Bridge>();
    instance->load(path, selected); *out = instance.release();
}); }
int clef_vst_scan(const char* path, void (*entry)(const char*, const char*, void*), void* context) noexcept { return guarded([&] {
    require(path && entry, "invalid scan arguments");
    std::string error;
    auto module = VST3::Hosting::Module::create(path, error);
    require(bool(module), error.c_str());
    for (auto& info : module->getFactory().classInfos()) {
        if (info.category() != kVstAudioEffectClass) continue;
        auto categories = info.subCategoriesString();
        bool instrument = categories.find("Instrument") != std::string::npos;
        // Legacy factories expose no subcategories; inspect their buses in this disposable process.
        if (categories.empty()) {
            auto component = module->getFactory().createInstance<IComponent>(info.ID());
            auto host = owned(new Host);
            if (component && success(component->initialize(host))) {
                instrument = component->getBusCount(kAudio, kInput) == 0 && component->getBusCount(kEvent, kInput) > 0;
                component->terminate();
            }
        }
        if (instrument) entry(info.ID().toString().c_str(), info.name().c_str(), context);
    }
}); }
int clef_vst_destroy(void* p) noexcept { return guarded([&] { bridge(p).main_thread(); delete &bridge(p); }); }
const char* clef_vst_name(void* p) noexcept { return p ? static_cast<Bridge*>(p)->name.c_str() : ""; }
int clef_vst_activate(void* p, int offline, double rate, uint32_t* latency) noexcept { return guarded([&] {
    auto& b = bridge(p); b.activate(offline != 0, rate); require(latency, "null latency output"); *latency = b.latency;
}); }
int clef_vst_deactivate(void* p) noexcept { return guarded([&] {
    auto& b = bridge(p); b.main_thread(); require(!b.processing.load(std::memory_order_acquire), "suspend VST3 audio before deactivation");
    if (b.active) { require(success(b.component->setActive(false)), "VST3 deactivation failed"); b.active = false; }
}); }
int clef_vst_process(void* p, uint64_t position, const Midi* events, size_t count, size_t frames, float* output) noexcept { return guarded([&] {
    require(output && (count == 0 || events), "null VST3 audio argument"); bridge(p).process(position, events, count, frames, output);
}); }
int clef_vst_suspend(void* p) noexcept { return guarded([&] { bridge(p).stop_audio(); }); }
int clef_vst_editor(void* p, void* parent, int open, int32_t* w, int32_t* h, int* resizable) noexcept { return guarded([&] {
    auto& b = bridge(p); b.main_thread(); b.close(); if (!open) return;
    require(parent && w && h && resizable, "invalid VST3 editor arguments");
    b.view = owned(b.controller->createView("editor")); require(bool(b.view), "VST3 editor unavailable");
    try {
        ViewRect rect {}; require(success(b.view->getSize(&rect)), "VST3 editor size unavailable");
        *w = rect.right-rect.left; *h = rect.bottom-rect.top;
        require(*w > 0 && *h > 0 && *w <= 8192 && *h <= 8192, "invalid VST3 editor dimensions");
        require(success(b.view->isPlatformTypeSupported(kPlatformTypeNSView)), "VST3 NSView embedding unsupported");
        require(success(b.view->setFrame(b.handler)), "VST3 editor rejected frame");
        require(success(b.view->attached(parent, kPlatformTypeNSView)), "VST3 editor attachment failed");
        *resizable = b.view->canResize() == kResultTrue;
    } catch (...) { b.view->setFrame(nullptr); b.view = nullptr; b.signals.resize.store(0); throw; }
}); }
int clef_vst_resize(void* p, int32_t* w, int32_t* h) noexcept { return guarded([&] {
    auto& b = bridge(p); b.main_thread(); require(w && h, "null VST3 resize argument"); b.resize(*w, *h);
}); }
int clef_vst_poll(void* p, uint32_t* flags, int32_t* w, int32_t* h) noexcept { return guarded([&] {
    auto& b = bridge(p); b.main_thread(); require(flags && w && h, "null VST3 poll argument");
    require(!b.signals.overflow.exchange(false), "VST3 parameter handoff overflow");
    Edit edit; while (b.signals.feedback.pop(edit))
        require(success(b.controller->setParamNormalized(edit.id, edit.value)), "VST3 controller feedback failed");
    *flags = b.signals.restart.exchange(0, std::memory_order_acq_rel);
    auto size = b.signals.resize.exchange(0, std::memory_order_acq_rel); *w = *h = 0;
    if (size && b.view) {
        *w = int32_t(size >> 32); *h = int32_t(size & UINT32_MAX);
        // canResize gates user dragging, not a plugin-originated scale request.
        ViewRect rect {0, 0, *w, *h};
        require(success(b.view->onSize(&rect)), "VST3 editor rejected its requested size");
    }
}); }
int clef_vst_save(void* p, const uint8_t** bytes, size_t* size) noexcept { return guarded([&] {
    auto& b = bridge(p); require(bytes && size, "null VST3 state output"); b.save(); *bytes = b.snapshot.data(); *size = b.snapshot.size();
}); }
int clef_vst_restore(void* p, const uint8_t* bytes, size_t size) noexcept { return guarded([&] {
    require(bytes || size == 0, "null VST3 state input"); bridge(p).restore(bytes, size);
}); }
}

#include <csignal>
namespace {
volatile std::sig_atomic_t render_cancelled = 0;
using SignalHandler = void (*)(int);
SignalHandler previous_interrupt = SIG_DFL;
void cancel_render(int) { render_cancelled = 1; }
}
extern "C" int clef_render_cancel_begin() noexcept {
    render_cancelled = 0;
    previous_interrupt = std::signal(SIGINT, cancel_render);
    return previous_interrupt == SIG_ERR ? -1 : 0;
}
extern "C" int clef_render_cancelled() noexcept { return render_cancelled != 0; }
extern "C" void clef_render_cancel_end() noexcept { std::signal(SIGINT, previous_interrupt); }
