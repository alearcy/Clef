#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <atomic>

namespace {
std::atomic<bool> quit_requested {false};
bool gate_active = false;
NSApplicationTerminateReply request_quit(id, SEL, NSApplication*) {
    if (!gate_active) return NSTerminateNow;
    quit_requested.store(true, std::memory_order_release);
    return NSTerminateCancel;
}
}

// Add only the missing lifecycle hook; winit retains its own delegate and all UI ownership.
extern "C" bool clef_install_quit_gate() noexcept {
    if (![NSThread isMainThread] || !NSApp.delegate) return false;
    Class delegate_class = object_getClass(NSApp.delegate);
    SEL selector = @selector(applicationShouldTerminate:);
    Method existing = class_getInstanceMethod(delegate_class, selector);
    if (existing && method_getImplementation(existing) != reinterpret_cast<IMP>(request_quit)) return false;
    if (!existing && !class_addMethod(delegate_class, selector, reinterpret_cast<IMP>(request_quit), "Q@:@")) return false;
    gate_active = true;
    return true;
}
extern "C" bool clef_take_quit_request() noexcept { return quit_requested.exchange(false, std::memory_order_acq_rel); }
extern "C" void clef_remove_quit_gate() noexcept { gate_active = false; }
