#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

namespace {
void disarm_layers(CALayer* layer) {
    for (CALayer* child in layer.sublayers) disarm_layers(child);
    // Discarded plugin layers must not call delegates whose C++ owners are destroyed next.
    layer.delegate = nil;
    [layer removeAllAnimations];
}
}

// A disposable ABI embedding surface, inside the eframe-owned view/window.
extern "C" void* clef_plugin_parent_create(void* raw_parent) noexcept {
    @autoreleasepool {
        NSView* parent = static_cast<NSView*>(raw_parent);
        if (!parent) return nullptr;
        NSView* surface = [[NSView alloc] initWithFrame:parent.bounds];
        surface.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        surface.wantsLayer = YES;
        [parent addSubview:surface];
        return surface;
    }
}
extern "C" void clef_plugin_parent_detach(void* raw_surface) noexcept {
    @autoreleasepool {
        NSView* surface = static_cast<NSView*>(raw_surface);
        disarm_layers(surface.layer);
        [surface removeFromSuperview];
        [CATransaction flush];
    }
}
extern "C" void clef_plugin_parent_destroy(void* raw_surface) noexcept {
    @autoreleasepool { [static_cast<NSView*>(raw_surface) release]; }
}

extern "C" void* clef_audio_activity_begin() noexcept {
    @autoreleasepool {
        return [[[NSProcessInfo processInfo]
            beginActivityWithOptions:NSActivityUserInitiated | NSActivityLatencyCritical
            reason:@"Realtime isolated plugin processing"] retain];
    }
}
extern "C" void clef_audio_activity_end(void* raw_activity) noexcept {
    @autoreleasepool {
        id activity = static_cast<id>(raw_activity);
        [[NSProcessInfo processInfo] endActivity:activity];
        [activity release];
    }
}
