use axum_extra::extract::CookieJar;

/// The default/fallback locale. English catalogs are the only ones shipped today; new locales
/// land as `locales/<code>.yml` plus an entry in `SUPPORTED_LOCALES`.
pub const DEFAULT_LOCALE: &str = "en";

/// Cookie set by a locale-override request; wins over `Accept-Language`.
pub const LOCALE_COOKIE_NAME: &str = "locale";

/// Locales this app can actually render. Resolution falls back to [`DEFAULT_LOCALE`] for
/// anything else.
const SUPPORTED_LOCALES: &[&str] = &[DEFAULT_LOCALE, "eo"];

/// Per-request translator handed to Askama templates. Templates call its methods
/// (`{{ tr.browse_title() }}`) because Askama can't invoke the `t!` macro itself.
#[derive(Clone)]
pub struct Tr {
    locale: String,
}

impl Tr {
    pub fn new(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
        }
    }

    /// English-only translator, for tests and non-request callers.
    pub fn english() -> Self {
        Self::new(DEFAULT_LOCALE)
    }

    fn s(&self, key: &str) -> String {
        rust_i18n::t!(key, locale = self.locale.as_str()).to_string()
    }

    /// Translates an arbitrary key - used for dynamic keys built at render time.
    #[allow(dead_code)]
    pub fn get(&self, key: &str) -> String {
        self.s(key)
    }

    /// Resolves the request's locale per the `web-frontend-localization` spec: cookie override,
    /// then `Accept-Language`, then English.
    pub fn resolve(jar: &CookieJar, accept_language: Option<&str>) -> Self {
        if let Some(cookie_locale) = jar.get(LOCALE_COOKIE_NAME).map(|c| c.value()) {
            if SUPPORTED_LOCALES.contains(&cookie_locale) {
                return Self::new(cookie_locale);
            }
        }
        if let Some(header) = accept_language {
            if let Some(tag) = header.split(',').next().and_then(|t| t.split(';').next()) {
                let tag = tag.trim();
                let base = tag.split('-').next().unwrap_or(tag);
                if SUPPORTED_LOCALES.contains(&base) {
                    return Self::new(base);
                }
            }
        }
        Self::english()
    }

    pub fn site_title(&self) -> String {
        self.s("site.title")
    }
    pub fn menu_log_in(&self) -> String {
        self.s("menu.log_in")
    }
    pub fn menu_log_out(&self) -> String {
        self.s("menu.log_out")
    }
    pub fn browse_title(&self) -> String {
        self.s("browse.title")
    }
    pub fn browse_search_label(&self) -> String {
        self.s("browse.search_label")
    }
    pub fn browse_search_placeholder(&self) -> String {
        self.s("browse.search_placeholder")
    }
    pub fn browse_search_submit(&self) -> String {
        self.s("browse.search_submit")
    }
    pub fn browse_empty(&self) -> String {
        self.s("browse.empty")
    }
    pub fn browse_prev(&self) -> String {
        self.s("browse.prev")
    }
    pub fn browse_next(&self) -> String {
        self.s("browse.next")
    }
    pub fn browse_add_new(&self) -> String {
        self.s("browse.add_new")
    }
    pub fn detail_edition(&self) -> String {
        self.s("detail.edition")
    }
    pub fn detail_publisher(&self) -> String {
        self.s("detail.publisher")
    }
    pub fn detail_notes(&self) -> String {
        self.s("detail.notes")
    }
    pub fn detail_tags(&self) -> String {
        self.s("detail.tags")
    }
    pub fn detail_version_history(&self) -> String {
        self.s("detail.version_history")
    }
    pub fn new_title(&self) -> String {
        self.s("new.title")
    }
    pub fn new_field_system_id(&self) -> String {
        self.s("new.field_system_id")
    }
    pub fn new_field_name(&self) -> String {
        self.s("new.field_name")
    }
    pub fn new_field_edition(&self) -> String {
        self.s("new.field_edition")
    }
    pub fn new_field_publisher(&self) -> String {
        self.s("new.field_publisher")
    }
    pub fn new_field_notes(&self) -> String {
        self.s("new.field_notes")
    }
    pub fn new_field_tags(&self) -> String {
        self.s("new.field_tags")
    }
    pub fn new_submit(&self) -> String {
        self.s("new.submit")
    }
    pub fn new_not_authorized(&self) -> String {
        self.s("new.not_authorized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_override_wins_over_accept_language() {
        let jar = CookieJar::new().add(("locale", "en"));
        let tr = Tr::resolve(&jar, Some("zz"));
        assert_eq!(tr.locale, "en");
    }

    #[test]
    fn unsupported_cookie_falls_through_to_accept_language() {
        let jar = CookieJar::new().add((LOCALE_COOKIE_NAME, "zz"));
        let tr = Tr::resolve(&jar, Some("en"));
        assert_eq!(tr.locale, "en");
    }

    #[test]
    fn accept_language_region_tag_matches_base_locale() {
        let tr = Tr::resolve(&CookieJar::new(), Some("en-GB,en;q=0.9"));
        assert_eq!(tr.locale, "en");
    }

    #[test]
    fn unsupported_accept_language_and_no_cookie_falls_back_to_english() {
        let tr = Tr::resolve(&CookieJar::new(), Some("fr-CA,fr;q=0.9"));
        assert_eq!(tr.locale, DEFAULT_LOCALE);
    }

    #[test]
    fn missing_everything_falls_back_to_english() {
        let tr = Tr::resolve(&CookieJar::new(), None);
        assert_eq!(tr.locale, DEFAULT_LOCALE);
    }

    #[test]
    fn english_translator_resolves_a_known_key() {
        assert!(!Tr::english().browse_title().is_empty());
    }
}
