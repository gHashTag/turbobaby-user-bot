// Pixel art CSS theme
//
// These classes will be referenced in the Dioxus RSX
// and matched with CSS in index.html

pub const CSS_VARS: &str = r#"
:root {
    --bg-primary: #0f0f1a;
    --bg-secondary: #1a1a2e;
    --bg-tertiary: #1a1a2e;
    --border: #2a2a4a;
    --text-primary: #E8E8E8;
    --text-secondary: #8b8b9e;
    --green: #39ff14;
    --cyan: #00e5ff;
    --yellow: #ffe600;
    --purple: #b388ff;
    --red: #ff4757;
    --pixel-shadow: 4px 4px 0 #000;
    --pixel-border: 2px solid var(--border);
}

/* Pixel art fonts */
@font-face {
    font-family: 'Inter';
    src: url('/assets/fonts/PressStart2P.woff2') format('woff2');
    font-display: swap;
}

/* Button styles */
.btn {
    font-family: 'Inter', monospace;
    border: var(--pixel-border);
    padding: 8px 16px;
    font-size: 11px;
    text-transform: uppercase;
    cursor: pointer;
    box-shadow: var(--pixel-shadow);
}

.btn-primary {
    background: var(--green);
    color: #000;
}

.btn-secondary {
    background: var(--cyan);
    color: #000;
}

.btn-outline {
    background: transparent;
    color: var(--green);
}

/* Card styles */
.card {
    background: var(--bg-tertiary);
    border: var(--pixel-border);
    box-shadow: var(--pixel-shadow);
    padding: 16px;
}

/* Navigation */
.bottom-nav {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    background: var(--bg-secondary);
    border-top: var(--pixel-border);
    display: flex;
    justify-content: space-around;
    padding: 8px 0;
}

/* Modal */
.modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
}

.modal {
    background: var(--bg-tertiary);
    border: var(--pixel-border);
    box-shadow: var(--pixel-shadow);
    max-width: 90vw;
}
"#;

pub fn inject_theme() -> String {
    format!("<style>{}</style>", CSS_VARS)
}
