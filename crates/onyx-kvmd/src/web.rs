pub const INDEX_HTML: &[u8] = include_bytes!("../static/index.html");
pub const FAVICON_SVG: &[u8] = include_bytes!("../static/favicon.svg");
pub const APP_CSS: &[u8] = include_bytes!("../static/app.css");
pub const APP_JS: &[u8] = include_bytes!("../static/app.js");

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn index_html_contains_required_elements() {
        let html = std::str::from_utf8(INDEX_HTML).expect("INDEX_HTML must be valid utf-8");
        assert!(html.contains("id=\"viewport\""));
        assert!(html.contains("id=\"kvm-canvas\""));
        assert!(html.contains("id=\"menu-dock\""));
        assert!(html.contains("id=\"menu-drawer\""));
        assert!(html.contains("id=\"auth-modal\""));
        assert!(html.contains("id=\"heartbeat-dot\""));
        assert!(html.contains("id=\"net-rate\""));
        assert!(html.contains("id=\"pair-code\""));
        assert!(html.contains("id=\"token-input\""));
        assert!(html.contains("id=\"close-drawer-btn\""));
        assert!(html.contains("id=\"menu-toggle-btn\""));
        assert!(html.contains("id=\"dock-copy-btn\""));
        assert!(html.contains("id=\"dock-name\""));
        assert!(html.contains("id=\"drawer-title\""));
        assert!(!html.contains("<h2>Onyx KVM Controls</h2>"));
        assert!(html.contains("id=\"dock-subsystems\""));
        assert!(html.contains("id=\"subsys-video\""));
        assert!(html.contains("id=\"subsys-hid\""));
        assert!(html.contains("id=\"subsys-mic\""));
        assert!(html.contains("id=\"subsys-speaker\""));
        assert!(html.contains("class=\"drawer-watermark\""));
        assert!(!html.contains("id=\"save-screenshot-btn\""));
        assert!(!html.contains("id=\"fetch-frame-btn\""));
        assert!(html.contains("id=\"opt-speed\""));
        assert!(html.contains("id=\"opt-format\""));
        assert!(html.contains("id=\"gate-video-btn\""));
        assert!(html.contains("id=\"gate-hid-btn\""));
        assert!(html.contains("id=\"gate-speaker-btn\""));
        assert!(html.contains("id=\"gate-mic-btn\""));
        assert!(html.contains("id=\"stream-audio-btn\""));
        assert!(html.contains("id=\"vu-meter-bar\""));
        assert!(html.contains("id=\"reset-defaults-btn\""));
        assert!(html.contains("id=\"reboot-btn\""));
        assert!(html.contains("id=\"change-pin-btn\""));
        assert!(html.contains("id=\"panel-setup\""));
        assert!(html.contains("id=\"setup-pin\""));
        assert!(html.contains("id=\"setup-btn\""));
        assert!(html.contains("id=\"stream-overlay\""));
        assert!(html.contains("id=\"stream-overlay-wake-btn\""));
        assert!(html.contains("id=\"wake-target-btn\""));
        assert!(html.contains("id=\"opt-keepawake\""));
        assert!(html.contains("id=\"pointer-capture-hint\""));
        assert!(html.contains("id=\"toast\""));
    }

    #[test]
    fn app_css_contains_edge_contraction_and_full_screen() {
        let css = std::str::from_utf8(APP_CSS).expect("APP_CSS must be valid utf-8");
        assert!(css.contains("body.menu-open-right #viewport"));
        assert!(css.contains("body.menu-open-left #viewport"));
        assert!(css.contains("body.menu-open-top #viewport"));
        assert!(css.contains("body.menu-open-bottom #viewport"));
        assert!(css.contains("body.unauth #viewport"));
        assert!(css.contains("#kvm-canvas"));
        assert!(css.contains(".stream-overlay"));
        assert!(css.contains(".btn-wake"));
        assert!(css.contains(".pointer-capture-hint"));
        assert!(css.contains(".dot-live"));
        assert!(css.contains(".dot-standby"));
        assert!(css.contains(".btn-gate"));
        assert!(css.contains(".vu-meter-bar"));
        assert!(css.contains(".btn-stream-start"));
        assert!(css.contains(".btn-stream-stop"));
        assert!(css.contains(".drawer-watermark"));
        assert!(css.contains(".dock-icon-btn"));
        assert!(css.contains(".dock-name"));
        assert!(css.contains(".dock-subsystems"));
        assert!(css.contains(".subsys-icon"));
        assert!(css.contains(".toast"));
    }

    #[test]
    fn app_js_contains_persistence_and_interactions() {
        let js = std::str::from_utf8(APP_JS).expect("APP_JS must be valid utf-8");
        assert!(js.contains("onyx_dock"));
        assert!(js.contains("onyx_token"));
        assert!(js.contains("onyx_res"));
        assert!(js.contains("onyx_format"));
        assert!(js.contains("onyx_fit"));
        assert!(js.contains("onyx_autostream"));
        assert!(js.contains("onyx_autohid"));
        assert!(js.contains("onyx_pointerlock"));
        assert!(js.contains("onyx_speed"));
        assert!(js.contains("onyx_keepawake"));
        assert!(js.contains("wakeTarget"));
        assert!(js.contains("startKeepAwakeTimer"));
        assert!(js.contains("VideoDecoder"));
        assert!(js.contains("renderVideoFrame"));
        assert!(js.contains("getVideoGeometry"));
        assert!(js.contains("setPointerCapture"));
        assert!(js.contains("applyDockPosition"));
        assert!(js.contains("toggleMenu"));
        assert!(js.contains("startStatsTimer"));
        assert!(js.contains("fetchFeatures"));
        assert!(js.contains("setFeature"));
        assert!(js.contains("startAudioStream"));
        assert!(js.contains("stopAudioStream"));
        assert!(js.contains("resetDefaults"));
        assert!(js.contains("rebootDevice"));
        assert!(js.contains("showStreamOverlay"));
        assert!(js.contains("hideStreamOverlay"));
        assert!(js.contains("copyScreenshotToClipboard"));
        assert!(!js.contains("saveScreenshotToFile"));
        assert!(js.contains("getCanvasBlob"));
        assert!(js.contains("preserveDrawingBuffer"));
        assert!(js.contains("updateSubsystemStatus"));
        assert!(js.contains("checkSetupStatus"));
        assert!(js.contains("setKvmName"));
        assert!(js.contains("setupClaim"));
        assert!(js.contains("applySetupMode"));
        assert!(js.contains("showToast"));
    }
}
