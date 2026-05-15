// Admin Panel Screen — Simple list of accessories
use dioxus::prelude::*;

/// Admin screen for accessories management
#[component]
pub fn AdminScreen() -> Element {
    rsx! {
        div { style: "min-height: 100vh; background: #0f0f1a; color: #e8e8e8; padding: 16px; padding-bottom: 80px;",
            h1 { style: "font-size: 24px; color: #ff4757; margin-bottom: 20px;",
                "🔧 Admin Panel"
            }

            div { style: "display: flex; flex-direction: column; gap: 10px; padding: 20px; background: #252540; border-radius: 8px;",
                div { style: "font-size: 24px;", "🔧" }
                div { style: "flex: 1;",
                    div { style: "font-weight: 700; font-size: 14px; margin-bottom: 4px;", "Grinder - 4 Piece" }
                    div { style: "font-size: 12px; color: #8b8b9e;",
                        "grinder • 890 ฿"
                    }
                }
            }

            div { style: "display: flex; flex-direction: column; gap: 10px; padding: 12px; background: #252540; border-radius: 8px;",
                div { style: "font-size: 24px;", "📄" }
                div { style: "flex: 1;",
                    div { style: "font-weight: 700; font-size: 14px; margin-bottom: 4px;", "Rolling Papers King Size" }
                    div { style: "font-size: 12px; color: #8b8b9e;",
                        "papers • 150 ฿"
                    }
                }
            }
        }
    }
}
