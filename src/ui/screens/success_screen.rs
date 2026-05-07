use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn SuccessScreen(id: String) -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            padding: 24px;
            text-align: center;
        ",
            // Success icon
            div { style: "
                font-size: 64px;
                margin-bottom: 20px;
                animation: pulse 1s ease-in-out infinite alternate;
            ", "✅" }

            // Title
            h1 { style: "
                font-size: 10px;
                color: #39ff14;
                text-shadow: 0 0 12px rgba(57,255,20,0.5);
                margin-bottom: 12px;
            ", "Order Placed!" }

            // Order ID
            p { style: "
                font-size: 12px;
                color: #8b8b9e;
                margin-bottom: 6px;
            ", "Your order #{id} has been received" }

            p { style: "
                font-size: 10px;
                color: #8b8b9e;
                margin-bottom: 32px;
            ", "We'll contact you shortly" }

            // Delivery estimate card
            div { style: "
                background: #1a1a2e;
                border: 2px solid #2a2a4a;
                border-radius: 8px;
                padding: 16px;
                width: 100%;
                max-width: 320px;
                margin-bottom: 24px;
                box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 12px; color: #00e5ff; margin-bottom: 10px;", "📦 Delivery Estimate" }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 10px;",
                    span { style: "color: #8b8b9e;", "Status:" }
                    span { style: "color: #39ff14;", "Confirmed" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 10px;",
                    span { style: "color: #8b8b9e;", "ETA:" }
                    span { "30-45 min" }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 10px;",
                    span { style: "color: #8b8b9e;", "Payment:" }
                    span { "Cash on delivery" }
                }
            }

            // Actions
            div { style: "display: flex; gap: 10px; width: 100%; max-width: 320px;",
                Link { to: Route::Menu {},
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 10px; flex: 1; padding: 12px;
                        background: #39ff14; color: #0f0f1a;
                        border: none; border-radius: 6px;
                        cursor: pointer; font-weight: bold;
                    ", "Back to Menu" }
                }
                Link { to: Route::Orders {},
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 10px; flex: 1; padding: 12px;
                        background: transparent; color: #e8e8e8;
                        border: 2px solid #2a2a4a; border-radius: 6px;
                        cursor: pointer;
                    ", "My Orders" }
                }
            }
        }
    }
}
