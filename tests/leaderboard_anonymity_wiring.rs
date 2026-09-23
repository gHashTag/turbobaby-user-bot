//! The public ride leaderboard must not publish a rider's Telegram id (2026-09-24).
//!
//! `src/ui/screens/ride_screen.rs` compiles only for wasm32, so its score body is guarded here as
//! text. Until 2026-09-24 it sent `"display_name":"Player {tid}"`, and the anonymous
//! `GET /api/game/high-scores` published that name verbatim. The server now masks such a name on
//! both routes (`public_display_name` in `src/api/game.rs`, unit-tested there); this guard keeps the
//! only shipped submitter from putting an identifier into the name again.

const RIDE_SCREEN: &str = include_str!("../src/ui/screens/ride_screen.rs");
const GAME_API: &str = include_str!("../src/api/game.rs");

#[test]
fn the_ride_screen_sends_no_display_name_built_from_the_id() {
    let score_lines: Vec<&str> = RIDE_SCREEN
        .lines()
        .filter(|l| l.contains("score_body") && l.contains("format!"))
        .collect();
    assert_eq!(score_lines.len(), 1, "exactly one score body is built");
    let body = score_lines[0];
    assert!(
        !body.contains("display_name"),
        "the score body carries no name: {body}"
    );
    assert!(
        !RIDE_SCREEN.contains("Player {tid}"),
        "no name is built from the Telegram id"
    );
}

#[test]
fn both_routes_pass_the_name_through_the_mask() {
    assert!(
        GAME_API.contains("\"display_name\": public_display_name(&stored, player)"),
        "the anonymous read masks the stored name"
    );
    assert!(
        GAME_API.contains(
            "let display_name = public_display_name(&req.display_name, Some(req.telegram_id));"
        ),
        "the write stores the masked name"
    );
    assert!(
        GAME_API.contains("SELECT telegram_id, display_name, high_score::bigint AS high_score"),
        "the read fetches the row's id so the mask can compare against it"
    );
}
