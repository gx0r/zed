use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    NativeLayerConfig, NativeLayerId, Pixels, Style, StyleRefinement, Styled, Window,
};
use refineable::Refineable;

/// An element that positions a native CALayer within the GPUI layout system.
///
/// This element participates in GPUI's flexbox layout and automatically updates
/// the native layer's bounds during the paint phase, keeping it synchronized
/// with the element's position and size.
///
/// # Platform Support
///
/// This feature is currently only implemented on **macOS**. On other platforms,
/// the element participates in layout but has no visual effect.
///
/// # Example
///
/// ```ignore
/// use std::ffi::c_void;
///
/// // Add a native layer to the window (e.g., AVPlayerLayer for video)
/// let layer_id = window.add_native_layer(
///     av_player_layer.as_ptr() as *mut c_void,
///     NativeLayerConfig::default(),
/// );
///
/// // Use the element in your render method
/// native_layer_element(layer_id)
///     .size_full()
///     .aspect_ratio(16.0 / 9.0)
///     .hidden(false)  // declaratively control visibility
///     .opacity(1.0)   // declaratively control opacity
/// ```
pub struct NativeLayerElement {
    layer_id: NativeLayerId,
    style: StyleRefinement,
    /// If set, automatically updates the native layer's hidden state during paint.
    hidden: Option<bool>,
    /// If set, automatically updates the native layer's opacity during paint.
    opacity: Option<f32>,
}

/// Create a new native layer element that positions the given native layer
/// according to GPUI's layout system.
pub fn native_layer_element(layer_id: NativeLayerId) -> NativeLayerElement {
    NativeLayerElement {
        layer_id,
        style: Default::default(),
        hidden: None,
        opacity: None,
    }
}

impl NativeLayerElement {
    /// Set whether the native layer should be hidden.
    /// The visibility will be automatically synced during paint.
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// Set the opacity of the native layer (0.0 to 1.0).
    /// The opacity will be automatically synced during paint.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity);
        self
    }
}

impl Element for NativeLayerElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        // Mark this layer as rendered this frame (for auto-hide of non-rendered layers)
        window.mark_native_layer_rendered(self.layer_id);

        // Update bounds
        window.update_native_layer_bounds(self.layer_id, bounds);

        let current = window.get_native_layer_config(self.layer_id);

        // Skip visibility/opacity updates if manual_visibility is enabled
        if current.manual_visibility {
            return;
        }

        // Visibility logic:
        // - If .hidden(true) was set explicitly, hide the layer
        // - Otherwise, show the layer (it's being rendered)
        let new_hidden = self.hidden.unwrap_or(false); // Default to visible when rendered
        let new_opacity = self.opacity.unwrap_or(current.opacity);

        // Only update if something actually changed
        if new_hidden != current.hidden || new_opacity != current.opacity {
            window.update_native_layer_config(
                self.layer_id,
                NativeLayerConfig {
                    z_order: current.z_order,
                    hidden: new_hidden,
                    opacity: new_opacity,
                    manual_visibility: false,
                },
            );
        }
    }
}

impl IntoElement for NativeLayerElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for NativeLayerElement {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        div, px, util::FluentBuilder, NativeLayerConfig, ParentElement, Render, TestAppContext,
        Window,
    };

    use super::*;

    struct NativeLayerTestView {
        layer_id: NativeLayerId,
    }

    impl Render for NativeLayerTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut crate::Context<Self>) -> impl crate::IntoElement {
            div()
                .size_full()
                .child(
                    native_layer_element(self.layer_id)
                        .w(px(200.0))
                        .h(px(100.0)),
                )
        }
    }

    /// Test view that can conditionally render or not render the native layer element
    struct ConditionalNativeLayerView {
        layer_id: NativeLayerId,
        should_render: bool,
        explicit_hidden: Option<bool>,
    }

    impl Render for ConditionalNativeLayerView {
        fn render(&mut self, _window: &mut Window, _cx: &mut crate::Context<Self>) -> impl crate::IntoElement {
            div().size_full().when(self.should_render, |el| {
                let mut layer_el = native_layer_element(self.layer_id)
                    .w(px(200.0))
                    .h(px(100.0));
                if let Some(hidden) = self.explicit_hidden {
                    layer_el = layer_el.hidden(hidden);
                }
                el.child(layer_el)
            })
        }
    }

    #[crate::test]
    fn test_native_layer_add_remove(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|window, _cx| {
            let layer_id = window.add_native_layer(
                std::ptr::null_mut(),
                NativeLayerConfig::default(),
            );
            NativeLayerTestView { layer_id }
        });
        cx.run_until_parked();
    }

    #[crate::test]
    fn test_native_layer_element_participates_in_layout(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|window, _cx| {
            let layer_id = window.add_native_layer(
                std::ptr::null_mut(),
                NativeLayerConfig::default(),
            );
            NativeLayerTestView { layer_id }
        });

        cx.run_until_parked();
    }

    #[crate::test]
    fn test_native_layer_auto_hides_when_not_rendered(cx: &mut TestAppContext) {
        let (view, mut cx) = cx.add_window_view(|window, _cx| {
            let layer_id = window.add_native_layer(
                std::ptr::null_mut(),
                NativeLayerConfig::default(),
            );
            ConditionalNativeLayerView {
                layer_id,
                should_render: true,
                explicit_hidden: None,
            }
        });
        cx.run_until_parked();

        // Layer should be visible when rendered
        let layer_id = view.read_with(cx, |view, _| view.layer_id);
        cx.update(|window, _| {
            let config = window.get_native_layer_config(layer_id);
            assert!(!config.hidden, "Layer should be visible when element is rendered");
        });

        // Stop rendering the element
        view.update(cx, |view, cx| {
            view.should_render = false;
            cx.notify();
        });
        cx.run_until_parked();

        // Need to trigger another frame for the auto-hide to take effect.
        // The reconciliation at frame N+1 hides layers not rendered in frame N.
        cx.update(|window, _| {
            window.refresh();
        });
        cx.run_until_parked();

        // After next frame, layer should be auto-hidden
        cx.update(|window, _| {
            let config = window.get_native_layer_config(layer_id);
            assert!(config.hidden, "Layer should be auto-hidden when element is not rendered");
        });
    }

    #[crate::test]
    fn test_native_layer_visible_when_rendered(cx: &mut TestAppContext) {
        let (view, mut cx) = cx.add_window_view(|window, _cx| {
            let layer_id = window.add_native_layer(
                std::ptr::null_mut(),
                NativeLayerConfig {
                    hidden: true, // Start hidden
                    ..Default::default()
                },
            );
            ConditionalNativeLayerView {
                layer_id,
                should_render: true,
                explicit_hidden: None,
            }
        });
        cx.run_until_parked();

        // Layer should become visible when rendered (even if it started hidden)
        let layer_id = view.read_with(cx, |view, _| view.layer_id);
        cx.update(|window, _| {
            let config = window.get_native_layer_config(layer_id);
            assert!(!config.hidden, "Layer should become visible when element renders");
        });
    }

    #[crate::test]
    fn test_native_layer_hidden_override_respected(cx: &mut TestAppContext) {
        let (view, mut cx) = cx.add_window_view(|window, _cx| {
            let layer_id = window.add_native_layer(
                std::ptr::null_mut(),
                NativeLayerConfig::default(),
            );
            ConditionalNativeLayerView {
                layer_id,
                should_render: true,
                explicit_hidden: Some(true), // Explicitly hide via .hidden(true)
            }
        });
        cx.run_until_parked();

        // Layer should be hidden because of explicit .hidden(true)
        let layer_id = view.read_with(cx, |view, _| view.layer_id);
        cx.update(|window, _| {
            let config = window.get_native_layer_config(layer_id);
            assert!(config.hidden, "Layer should be hidden when .hidden(true) is set");
        });

        // Remove the explicit hidden override
        view.update(cx, |view, cx| {
            view.explicit_hidden = Some(false);
            cx.notify();
        });
        cx.run_until_parked();

        // Layer should now be visible
        cx.update(|window, _| {
            let config = window.get_native_layer_config(layer_id);
            assert!(!config.hidden, "Layer should be visible when .hidden(false) is set");
        });
    }

    #[crate::test]
    fn test_native_layer_manual_visibility_prevents_auto_hide(cx: &mut TestAppContext) {
        let (view, mut cx) = cx.add_window_view(|window, _cx| {
            let layer_id = window.add_native_layer(
                std::ptr::null_mut(),
                NativeLayerConfig {
                    manual_visibility: true, // Opt out of auto-hide
                    hidden: false,
                    ..Default::default()
                },
            );
            ConditionalNativeLayerView {
                layer_id,
                should_render: true,
                explicit_hidden: None,
            }
        });
        cx.run_until_parked();

        let layer_id = view.read_with(cx, |view, _| view.layer_id);

        // Stop rendering the element
        view.update(cx, |view, cx| {
            view.should_render = false;
            cx.notify();
        });
        cx.run_until_parked();

        // Layer should NOT be auto-hidden because manual_visibility is true
        cx.update(|window, _| {
            let config = window.get_native_layer_config(layer_id);
            assert!(!config.hidden, "Layer with manual_visibility should not be auto-hidden");
        });
    }
}
