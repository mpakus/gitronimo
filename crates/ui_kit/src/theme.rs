use gpui::{Rgba, rgb};

#[derive(Clone, Copy)]
pub struct Theme {
    pub colors: ThemeColors,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Copy)]
pub struct ThemeColors {
    pub toolbar_background: Rgba,
    pub search_field_background: Rgba,
    pub list_row_border: Rgba,
    pub window_background: Rgba,
    pub panel_background: Rgba,
    pub raised_background: Rgba,
    pub sidebar_background: Rgba,
    pub border: Rgba,
    pub separator: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    /// Hover state for accent-filled controls.
    pub accent_hover: Rgba,
    /// Label color that stays readable on top of `accent`.
    pub accent_foreground: Rgba,
    pub selection: Rgba,
    pub focus_ring: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub added_line: Rgba,
    pub removed_line: Rgba,
    pub modified_line: Rgba,
    pub conflict: Rgba,
    pub overlay_scrim: Rgba,
    pub graph_lanes: [Rgba; 6],
}

impl Theme {
    #[must_use]
    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self::light(),
            Appearance::Dark => Self::dark(),
        }
    }

    #[must_use]
    pub fn light() -> Self {
        Self {
            colors: ThemeColors {
                toolbar_background: rgb(0xf0_f3_f7),
                search_field_background: rgb(0xeb_f0_f5),
                list_row_border: rgb(0xdd_e4_ec),
                window_background: rgb(0xf5_f7_fa),
                panel_background: rgb(0xff_ff_ff),
                raised_background: rgb(0xeb_f0_f5),
                sidebar_background: rgb(0xe9_ef_f5),
                border: rgb(0xcd_d6_e0),
                separator: rgb(0xdd_e4_ec),
                text_primary: rgb(0x1a_24_30),
                text_secondary: rgb(0x4b_5d_70),
                text_muted: rgb(0x5e_6e_7e),
                accent: rgb(0x00_68_cc),
                accent_hover: rgb(0x00_57_ad),
                accent_foreground: rgb(0xff_ff_ff),
                selection: rgb(0xd6_e8_f8),
                focus_ring: rgb(0x00_7a_eb),
                success: rgb(0x12_8a_4b),
                warning: rgb(0xa8_60_00),
                danger: rgb(0xc7_28_3b),
                added_line: rgb(0xd9_f3_e4),
                removed_line: rgb(0xfb_df_e1),
                modified_line: rgb(0xff_f1_c7),
                conflict: rgb(0xff_e4_b3),
                overlay_scrim: gpui::hsla(0.0, 0.0, 0.0, 0.25).into(),
                graph_lanes: [
                    rgb(0x00_68_cc),
                    rgb(0x12_8a_4b),
                    rgb(0xa8_60_00),
                    rgb(0x7a_43_c8),
                    rgb(0x00_85_7a),
                    rgb(0xc7_28_3b),
                ],
            },
        }
    }
    #[must_use]
    pub fn dark() -> Self {
        Self {
            colors: ThemeColors {
                toolbar_background: rgb(0x1a_21_2a),
                search_field_background: rgb(0x29_34_42),
                list_row_border: rgb(0x2a_36_45),
                window_background: rgb(0x16_1b_22),
                panel_background: rgb(0x1f_27_33),
                raised_background: rgb(0x29_34_42),
                sidebar_background: rgb(0x12_17_1e),
                border: rgb(0x35_43_54),
                separator: rgb(0x2a_36_45),
                text_primary: rgb(0xe8_ed_f2),
                text_secondary: rgb(0xb1_bd_cb),
                text_muted: rgb(0x7e_8c_9d),
                accent: rgb(0x43_9a_ff),
                accent_hover: rgb(0x6a_b1_ff),
                accent_foreground: rgb(0x0d_16_22),
                selection: rgb(0x2a_3f_5c),
                focus_ring: rgb(0x77_ba_ff),
                success: rgb(0x49_c6_7c),
                warning: rgb(0xf0_b5_4d),
                danger: rgb(0xf1_6e_78),
                added_line: rgb(0x1f_56_43),
                removed_line: rgb(0x65_35_3d),
                modified_line: rgb(0x59_49_28),
                conflict: rgb(0x7f_50_23),
                overlay_scrim: gpui::hsla(0.0, 0.0, 0.0, 0.45).into(),
                graph_lanes: [
                    rgb(0x43_9a_ff),
                    rgb(0x49_c6_7c),
                    rgb(0xf0_b5_4d),
                    rgb(0xc5_8a_ff),
                    rgb(0x45_ce_c1),
                    rgb(0xf1_6e_78),
                ],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Appearance, Theme};

    #[test]
    fn dark_theme_distinguishes_the_window_and_panel() {
        let colors = Theme::dark().colors;

        assert_ne!(colors.window_background, colors.panel_background);
    }

    #[test]
    fn appearances_have_distinct_window_backgrounds() {
        assert_ne!(
            Theme::for_appearance(Appearance::Light)
                .colors
                .window_background,
            Theme::for_appearance(Appearance::Dark)
                .colors
                .window_background
        );
    }
}
