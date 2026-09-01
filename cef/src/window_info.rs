use crate::{sys::cef_window_handle_t, *};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::*;

impl WindowInfo {
    /// Create the browser as a child window.
    pub fn set_as_child(self, parent: cef_window_handle_t, bounds: &Rect) -> Self {
        Self {
            #[cfg(target_os = "windows")]
            style: WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_TABSTOP | WS_VISIBLE,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            parent_window: parent,
            #[cfg(target_os = "macos")]
            parent_view: parent,
            bounds: bounds.clone(),
            #[cfg(target_os = "macos")]
            hidden: 0,
            ..self
        }
    }

    /// Create the browser as a child window, additionally naming the client's
    /// `xdg_surface` when the active Ozone platform is Wayland.
    ///
    /// `parent` is a `wl_surface*` under Ozone/Wayland or an X11 `Window` under
    /// Ozone/X11 (same as [`WindowInfo::set_as_child`]). `parent_xdg` is the
    /// `xdg_surface*` that `parent` belongs to; it anchors the browser's popups,
    /// `<select>` dropdowns and tooltips, and may be `None` if the host toolkit
    /// doesn't expose it, in which case popups fall back to a degraded form. It
    /// is ignored under X11.
    ///
    /// Only available on `x86_64-unknown-linux-gnu`: the `parent_xdg_surface`
    /// field comes from our Wayland patch on top of upstream CEF, and only the
    /// x86_64 Linux CEF archive (built from `cef-wayland-build`) carries it --
    /// the official prebuilt archives used for other Linux architectures don't.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub fn set_as_child_wayland(
        self,
        parent: cef_window_handle_t,
        parent_xdg: Option<sys::cef_xdg_surface_handle_t>,
        bounds: &Rect,
    ) -> Self {
        Self {
            parent_window: parent,
            parent_xdg_surface: parent_xdg.unwrap_or(std::ptr::null_mut()),
            bounds: bounds.clone(),
            ..self
        }
    }

    /// Create the browser as a popup window.
    #[cfg(target_os = "windows")]
    pub fn set_as_popup(self, parent: cef_window_handle_t, title: &str) -> Self {
        Self {
            window_name: CefString::from(title),
            parent_window: parent,
            style: WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_VISIBLE,
            bounds: Rect {
                x: CW_USEDEFAULT,
                y: CW_USEDEFAULT,
                width: CW_USEDEFAULT,
                height: CW_USEDEFAULT,
            },
            ..self
        }
    }

    /// Create the browser using windowless (off-screen) rendering. No window
    /// will be created for the browser and all rendering will occur via the
    /// CefRenderHandler interface. The |parent| value will be used to identify
    /// monitor info and to act as the parent window for dialogs, context menus,
    /// etc. If |parent| is not provided then the main screen monitor will be used
    /// and some functionality that requires a parent window may not function
    /// correctly. In order to create windowless browsers the
    /// CefSettings.windowless_rendering_enabled value must be set to true.
    /// Transparent painting is enabled by default but can be disabled by setting
    /// CefBrowserSettings.background_color to an opaque value.
    pub fn set_as_windowless(self, parent: cef_window_handle_t) -> Self {
        Self {
            windowless_rendering_enabled: 1,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            parent_window: parent,
            #[cfg(target_os = "macos")]
            parent_view: parent,
            runtime_style: RuntimeStyle::ALLOY,
            ..self
        }
    }
}
