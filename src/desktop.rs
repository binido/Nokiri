use eframe::egui::{self, ViewportCommand};
use nokiri::worker::{Shutdown, Snapshot};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSImage, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusItem, NSWindow,
};
use objc2_foundation::{
    MainThreadMarker, NSAppleEventDescriptor, NSAppleEventManager, NSNotification,
    NSNotificationCenter, NSObject, NSObjectProtocol, NSSize, NSString, ns_string,
};
use std::cell::Cell;

const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const REOPEN_APPLICATION: u32 = u32::from_be_bytes(*b"rapp");

struct MenuState {
    ctx: egui::Context,
    window: Retained<NSWindow>,
    toggle: Cell<bool>,
    quitting: Cell<bool>,
    background: Cell<bool>,
    shutdown: Shutdown,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuState]
    struct MenuTarget;

    unsafe impl NSObjectProtocol for MenuTarget {}

    impl MenuTarget {
        #[unsafe(method(open:))]
        fn open(&self, _sender: Option<&NSObject>) {
            self.show();
        }

        #[unsafe(method(reopen:reply:))]
        fn reopen(&self, _event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            self.show();
        }

        #[unsafe(method(toggle:))]
        fn toggle(&self, _sender: Option<&NSObject>) {
            self.ivars().toggle.set(true);
            self.ivars().ctx.request_repaint();
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&NSObject>) {
            self.ivars().quitting.set(true);
            self.ivars().ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        #[unsafe(method(willTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.ivars().shutdown.stop();
        }
    }
);

impl MenuTarget {
    fn show(&self) {
        self.ivars().background.set(false);
        let app = NSApplication::sharedApplication(self.mtm());
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        self.ivars().window.setAlphaValue(1.0);
        self.ivars().window.deminiaturize(None);
        self.ivars().window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        self.ivars()
            .ctx
            .send_viewport_cmd(ViewportCommand::Visible(true));
        self.ivars().ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
}

pub struct Desktop {
    target: Retained<MenuTarget>,
    item: Retained<NSStatusItem>,
    status: Retained<NSMenuItem>,
    detail: Retained<NSMenuItem>,
    toggle: Retained<NSMenuItem>,
    first_frame: bool,
    last_status: String,
}

impl Desktop {
    pub fn new(ctx: egui::Context, shutdown: Shutdown, background: bool) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new().expect("desktop integration runs on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        let window = app
            .windows()
            .iter()
            .find(|w| w.title().to_string() == "Nokiri")
            .ok_or_else(|| anyhow::anyhow!("Could not find Nokiri's window"))?;
        if background {
            // eframe shows the root window after its first paint, even for a hidden viewport.
            window.setAlphaValue(0.0);
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        }
        let target = MenuTarget::alloc(mtm).set_ivars(MenuState {
            ctx,
            window,
            toggle: Cell::new(false),
            quitting: Cell::new(false),
            background: Cell::new(background),
            shutdown,
        });
        let target: Retained<MenuTarget> = unsafe { msg_send![super(target), init] };
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Nokiri"));
        menu.setAutoenablesItems(false);
        let status = menu_item(mtm, "Nokiri: Loading...", None, &target);
        status.setEnabled(false);
        let detail = menu_item(mtm, "", None, &target);
        detail.setHidden(true);
        detail.setEnabled(false);
        let open = menu_item(mtm, "Open Nokiri", Some(sel!(open:)), &target);
        let toggle = menu_item(mtm, "Connect", Some(sel!(toggle:)), &target);
        toggle.setEnabled(false);
        let quit = menu_item(mtm, "Quit Nokiri", Some(sel!(quit:)), &target);
        for entry in [
            &status,
            &detail,
            &NSMenuItem::separatorItem(mtm),
            &open,
            &toggle,
            &NSMenuItem::separatorItem(mtm),
            &quit,
        ] {
            menu.addItem(entry);
        }
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        item.setMenu(Some(&menu));
        if let Some(button) = item.button(mtm) {
            button.setTitle(ns_string!("N"));
        }
        if let Some(main_menu) = app.mainMenu() {
            retarget_quit(&main_menu, &target);
        }
        unsafe {
            let _: () = msg_send![
                &*NSAppleEventManager::sharedAppleEventManager(),
                setEventHandler: &*target,
                andSelector: sel!(reopen:reply:),
                forEventClass: CORE_EVENT_CLASS,
                andEventID: REOPEN_APPLICATION
            ];
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &target,
                sel!(willTerminate:),
                Some(ns_string!("NSApplicationWillTerminateNotification")),
                None,
            );
        }
        Ok(Self {
            target,
            item,
            status,
            detail,
            toggle,
            first_frame: true,
            last_status: String::new(),
        })
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        snapshot: &Snapshot,
        initialized: bool,
        available: bool,
    ) -> bool {
        if self.first_frame {
            self.first_frame = false;
            ctx.request_repaint();
        } else if self.target.ivars().background.replace(false) {
            self.hide(ctx);
            self.target.ivars().window.setAlphaValue(1.0);
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.target.ivars().quitting.get()
            && !self.target.ivars().shutdown.requested()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide(ctx);
        }
        let state = if snapshot.error {
            "Error"
        } else if !initialized {
            "Loading"
        } else if snapshot.busy || !available {
            "Working"
        } else if snapshot.running {
            "Connected"
        } else {
            "Stopped"
        };
        self.status
            .setTitle(&NSString::from_str(&format!("Nokiri: {state}")));
        self.detail.setHidden(!snapshot.error);
        if snapshot.error {
            let summary: String = snapshot.message.chars().take(120).collect();
            self.detail.setTitle(&NSString::from_str(&summary));
            self.detail
                .setToolTip(Some(&NSString::from_str(&snapshot.message)));
        }
        self.toggle.setTitle(if snapshot.running {
            ns_string!("Disconnect")
        } else {
            ns_string!("Connect")
        });
        self.toggle.setEnabled(
            available && (snapshot.running || snapshot.state.selected_server().is_some()),
        );
        if self.last_status != state {
            self.last_status = state.into();
            let symbol = if snapshot.error {
                "exclamationmark.shield"
            } else if snapshot.running {
                "network"
            } else {
                "pause.circle"
            };
            if let Some(button) = self.item.button(self.target.mtm()) {
                if let Some(image) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str(symbol),
                    Some(&NSString::from_str(&format!("Nokiri: {state}"))),
                ) {
                    image.setTemplate(true);
                    image.setSize(NSSize::new(18.0, 18.0));
                    button.setImage(Some(&image));
                    button.setTitle(ns_string!(""));
                }
                button.setToolTip(Some(&NSString::from_str(&format!("Nokiri: {state}"))));
            }
        }
        self.target.ivars().toggle.replace(false)
    }

    fn hide(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.target.ivars().window.orderOut(None);
        NSApplication::sharedApplication(self.target.mtm())
            .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![&*NSAppleEventManager::sharedAppleEventManager(),
                removeEventHandlerForEventClass: CORE_EVENT_CLASS, andEventID: REOPEN_APPLICATION];
            NSNotificationCenter::defaultCenter().removeObserver(&self.target);
        }
        NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
    }
}

fn menu_item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<objc2::runtime::Sel>,
    target: &MenuTarget,
) -> Retained<NSMenuItem> {
    unsafe {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            ns_string!(""),
        );
        item.setTarget(Some(target));
        item
    }
}

fn retarget_quit(menu: &NSMenu, target: &MenuTarget) {
    for item in menu.itemArray() {
        if item.action() == Some(sel!(terminate:)) {
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(quit:)));
            }
        }
        if let Some(submenu) = item.submenu() {
            retarget_quit(&submenu, target);
        }
    }
}
