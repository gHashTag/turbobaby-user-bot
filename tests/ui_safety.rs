// UI safety tests: NaN/Inf guards, URL validation, overflow prevention

#[cfg(test)]
mod ui_safety_tests {
    // Helper mirroring the frontend guard logic
    fn safe_price_display(price: f64) -> String {
        if price.is_finite() {
            format!("{:.0}", price.max(0.0))
        } else {
            "0".to_string()
        }
    }

    fn is_safe_image_url(url: &str) -> bool {
        !url.is_empty()
            && (url.starts_with("http://")
                || url.starts_with("https://")
                || (url.starts_with('/') && !url.starts_with("//")))
    }

    #[test]
    fn nan_price_renders_as_zero() {
        assert_eq!(safe_price_display(f64::NAN), "0");
    }

    #[test]
    fn inf_price_renders_as_zero() {
        assert_eq!(safe_price_display(f64::INFINITY), "0");
    }

    #[test]
    fn neg_inf_price_renders_as_zero() {
        assert_eq!(safe_price_display(f64::NEG_INFINITY), "0");
    }

    #[test]
    fn negative_price_is_clamped_to_zero() {
        assert_eq!(safe_price_display(-10.0), "0");
    }

    #[test]
    fn normal_price_renders_correctly() {
        assert_eq!(safe_price_display(350.0), "350");
    }

    #[test]
    fn fractional_price_rounds() {
        assert_eq!(safe_price_display(349.9), "350");
    }

    #[test]
    fn valid_http_url_accepted() {
        assert!(is_safe_image_url("http://example.com/img.jpg"));
    }

    #[test]
    fn valid_https_url_accepted() {
        assert!(is_safe_image_url("https://example.com/img.jpg"));
    }

    #[test]
    fn relative_url_accepted() {
        assert!(is_safe_image_url("/assets/img.jpg"));
    }

    #[test]
    fn javascript_url_rejected() {
        assert!(!is_safe_image_url("javascript:alert(1)"));
    }

    #[test]
    fn data_url_rejected() {
        assert!(!is_safe_image_url("data:text/html,<script>"));
    }

    #[test]
    fn empty_url_rejected() {
        assert!(!is_safe_image_url(""));
    }

    #[test]
    fn ftp_url_rejected() {
        assert!(!is_safe_image_url("ftp://example.com/img.jpg"));
    }

    #[test]
    fn protocol_relative_url_rejected() {
        assert!(!is_safe_image_url("//evil.com/img.jpg"));
    }

    // Game bounds/overflow helpers
    fn safe_bud_type_idx(raw: u8, len: usize) -> usize {
        (raw as usize).min(len.saturating_sub(1))
    }

    #[test]
    fn bud_type_idx_within_bounds() {
        assert_eq!(safe_bud_type_idx(0, 4), 0);
        assert_eq!(safe_bud_type_idx(3, 4), 3);
    }

    #[test]
    fn bud_type_idx_clamped_above_bounds() {
        assert_eq!(safe_bud_type_idx(255, 4), 3);
    }

    #[test]
    fn bud_type_idx_empty_array() {
        assert_eq!(safe_bud_type_idx(0, 0), 0);
    }

    #[test]
    fn u32_spawn_tick_saturating_add() {
        let mut v = u32::MAX - 1;
        v = v.saturating_add(1);
        assert_eq!(v, u32::MAX);
        v = v.saturating_add(1);
        assert_eq!(v, u32::MAX);
    }

    #[test]
    fn u32_level_multiplier_no_panic() {
        let level = u32::MAX;
        let result = level.saturating_mul(50);
        assert_eq!(result, u32::MAX);
    }
}
