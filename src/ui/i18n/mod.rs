// Internationalization Module
use crate::ui::state::Language;
use dioxus::prelude::*;

#[derive(Clone)]
pub struct I18n {
    lang: Language,
}

impl I18n {
    pub fn new(lang: Language) -> Self {
        Self { lang }
    }

    pub fn t(&self, key: TranslationKey) -> &'static str {
        match (self.lang, key) {
            // Common translations
            (_, TranslationKey::Common(CommonKey::Add)) => "Add",
            (_, TranslationKey::Common(CommonKey::Remove)) => "Remove",
            (_, TranslationKey::Common(CommonKey::Save)) => "Save",
            (_, TranslationKey::Common(CommonKey::Cancel)) => "Cancel",
            (_, TranslationKey::Common(CommonKey::Confirm)) => "Confirm",
            (_, TranslationKey::Common(CommonKey::Back)) => "Back",
            (_, TranslationKey::Common(CommonKey::Next)) => "Next",
            (_, TranslationKey::Common(CommonKey::Finish)) => "Finish",
            // Page translations
            (_, TranslationKey::Home) => "Home",
            (_, TranslationKey::Menu) => "Menu",
            (_, TranslationKey::Cart) => "Cart",
            (_, TranslationKey::Garden) => "Garden",
            (_, TranslationKey::Quest) => "Quest",
            (_, TranslationKey::Profile) => "Profile",
            (_, TranslationKey::Orders) => "Orders",
            (_, TranslationKey::Checkout) => "Checkout",
            (_, TranslationKey::Loading) => "Loading...",
            (_, TranslationKey::Error) => "Error",
            (_, TranslationKey::Retry) => "Retry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TranslationKey {
    Common(CommonKey),
    Home,
    Menu,
    Cart,
    Garden,
    Quest,
    Profile,
    Orders,
    Checkout,
    Loading,
    Error,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommonKey {
    Add,
    Remove,
    Save,
    Cancel,
    Confirm,
    Back,
    Next,
    Finish,
}

#[component]
pub fn I18nProvider(children: Element) -> Element {
    let _language = use_context::<Signal<Language>>();

    rsx! {
        {children}
    }
}
