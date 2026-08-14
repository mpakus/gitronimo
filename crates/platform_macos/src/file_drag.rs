//! `AppKit` drag-out of existing files so other macOS apps can open the drop.

use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSPasteboardWriting, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};

const DRAG_ICON_SIZE: f64 = 32.0;

define_class!(
    /// Copy-only dragging source. Lives for the process so `AppKit` can query it
    /// after `beginDraggingSession` returns.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "GitRonimoFileDragSource"]
    struct FileDragSource;

    unsafe impl NSObjectProtocol for FileDragSource {}

    unsafe impl NSDraggingSource for FileDragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn dragging_session_source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy
        }
    }
);

thread_local! {
    static DRAG_SOURCE: RefCell<Option<Retained<FileDragSource>>> = const { RefCell::new(None) };
}

impl FileDragSource {
    fn shared() -> Option<Retained<Self>> {
        let mtm = objc2::MainThreadMarker::new()?;
        Some(DRAG_SOURCE.with(|cell| {
            if let Some(existing) = cell.borrow().clone() {
                return existing;
            }
            let allocated = Self::alloc(mtm).set_ivars(());
            let source: Retained<Self> = unsafe { msg_send![super(allocated), init] };
            *cell.borrow_mut() = Some(source.clone());
            source
        }))
    }
}

/// Starts an `AppKit` file-URL drag for `paths`. Call from a mouse-drag handler
/// on the main thread. Returns whether a session was started.
pub fn begin_external_file_drag(paths: &[impl AsRef<Path>]) -> bool {
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return false;
    };
    let paths: Vec<&Path> = paths
        .iter()
        .map(AsRef::as_ref)
        .filter(|path| path.exists())
        .collect();
    if paths.is_empty() {
        return false;
    }
    let app = NSApplication::sharedApplication(mtm);
    let Some(event) = app.currentEvent() else {
        return false;
    };
    let Some(window) = event.window(mtm) else {
        return false;
    };
    let Some(view) = window.contentView() else {
        return false;
    };
    let Some(source) = FileDragSource::shared() else {
        return false;
    };
    let location = view.convertPoint_fromView(event.locationInWindow(), None);
    let frame = NSRect::new(
        NSPoint::new(
            location.x - DRAG_ICON_SIZE / 2.0,
            location.y - DRAG_ICON_SIZE / 2.0,
        ),
        NSSize::new(DRAG_ICON_SIZE, DRAG_ICON_SIZE),
    );
    let workspace = NSWorkspace::sharedWorkspace();
    let mut items = Vec::with_capacity(paths.len());
    for path in paths {
        let Some(path_str) = path.to_str() else {
            continue;
        };
        let ns_path = NSString::from_str(path_str);
        let url = NSURL::fileURLWithPath(&ns_path);
        let writer = ProtocolObject::<dyn NSPasteboardWriting>::from_ref(&*url);
        let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);
        let icon = workspace.iconForFile(&ns_path);
        icon.setSize(NSSize::new(DRAG_ICON_SIZE, DRAG_ICON_SIZE));
        unsafe {
            item.setDraggingFrame_contents(frame, Some(AsRef::<AnyObject>::as_ref(&*icon)));
        }
        items.push(item);
    }
    if items.is_empty() {
        return false;
    }
    let item_array = NSArray::from_retained_slice(&items);
    let source = ProtocolObject::<dyn NSDraggingSource>::from_ref(&*source);
    let _session = view.beginDraggingSessionWithItems_event_source(&item_array, &event, source);
    true
}

#[cfg(test)]
mod tests {
    use super::begin_external_file_drag;
    use std::path::PathBuf;

    #[test]
    fn empty_path_list_does_not_start_a_drag() {
        let paths: [PathBuf; 0] = [];
        assert!(!begin_external_file_drag(&paths));
    }
}
