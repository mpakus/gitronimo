//! Overlay and composer motion helpers (GPUI `Animation`).
//!
//! Entrance timing is adapted from rgitui's MIT `Modal` (150ms ease-out).
//! `GitComet` is AGPL — approach-only.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ElementId, IntoElement, div, ease_out_quint, prelude::*,
};

use crate::app_state::{GitronimoApp, OverlaySlot};

/// Fade duration for modal overlays (open and close).
pub(crate) const OVERLAY_FADE_DURATION: Duration = Duration::from_millis(150);

/// Height + opacity duration for the commit composer details panel.
pub(crate) const COMPOSER_REVEAL_DURATION: Duration = Duration::from_millis(200);

/// Body (4×32) + `gap_2` (8) + options row (~22).
pub(crate) const COMPOSER_DETAILS_HEIGHT: f32 = 158.0;

pub(crate) fn overlay_fade(
    slot: OverlaySlot,
    closing: bool,
    generation: u64,
    element: impl IntoElement,
) -> AnyElement {
    let phase = if closing { "out" } else { "in" };
    let id = ElementId::Name(
        format!("overlay-fade-{}-{phase}-{generation}", slot.animation_key()).into(),
    );
    let animation = Animation::new(OVERLAY_FADE_DURATION).with_easing(ease_out_quint());
    let wrapper = div().absolute().inset_0().child(element);
    if closing {
        wrapper
            .with_animation(id, animation, |el, delta| el.opacity(1.0 - delta))
            .into_any_element()
    } else {
        wrapper
            .with_animation(id, animation, gpui::Styled::opacity)
            .into_any_element()
    }
}

impl GitronimoApp {
    pub(crate) fn fading_overlay(&self, slot: OverlaySlot, overlay: AnyElement) -> AnyElement {
        overlay_fade(
            slot,
            self.overlay_fade_out == Some(slot),
            self.overlay_fade_generation,
            overlay,
        )
    }
}
