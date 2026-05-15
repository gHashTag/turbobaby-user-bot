use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn SuccessScreen(id: String) -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            padding: 24px;
            text-align: center;
        ",
            // Success icon
            div { style: "
                font-size: 70px;
                margin-bottom: 20px;
                animation: pulse 1s ease-in-out infinite alternate;
            ", "✅" }

            // Title
            h1 { style: "
                font-size: 24px;
                font-weight: 800;
                color: #39ff14;
                text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5);
                letter-spacing: 2px;
                margin-bottom: 12px;
            ", "Order Placed!" }

            // Order ID
            p { style: "
                font-size: 15px;
                color: #8b8b9e;
                margin-bottom: 6px;
            ", "Your order #{id} has been received" }

            p { style: "
                font-size: 13px;
                color: #8b8b9e;
                margin-bottom: 32px;
            ", "We'll contact you shortly" }

            // Delivery estimate card
            div { style: "
                background: #16213e;
                border: 4px solid #2a2a4a;
                border-radius: 0;
                padding: 16px;
                width: 100%;
                max-width: 320px;
                margin-bottom: 24px;
                box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "📦 Delivery Estimate" }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "Status:" }
                    span { style: "color: #39ff14;", "Confirmed" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "ETA:" }
                    span { "30-45 min" }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "Payment:" }
                    span { "Cash on delivery" }
                }
            }

            // Actions
            div { style: "display: flex; gap: 10px; width: 100%; max-width: 320px;",
                Link { to: Route::Menu {},
                    button { style: "
                        font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                        background: #39ff14; color: #000;
                        border: 4px solid #2d9e0f; border-radius: 0;
                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ", "Back to Menu" }
                }
                Link { to: Route::Orders {},
                    button { style: "
                        font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                        background: transparent; color: #e8e8e8;
                        border: 4px solid #2a2a4a; border-radius: 0;
                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ", "My Orders" }
                }
            }
        }
    }
}
