//! `/uploads` serves only what the rental data references, and this file
//! holds the live router and read path to that.
//!
//! Owner, 2026-09-26, asked about the previous shop's media still reachable by
//! a direct link, verbatim: «Зачем они вообще нужны мне?». The operator read it
//! as: stop serving them; deleting the files is the owner's own irreversible
//! act and is not done here. The upload's local branch still writes to the
//! folder whenever no object store is configured, so the read path stays, and
//! since that day serves a name only when a bike's stored picture is exactly
//! `/uploads/<name>`. `specs/turbobaby/upload_media.t27` records it
//! (`OWNER_MEDIA_ANSWER_*`, `LOCAL_READ_*`); the rule is unit-tested and run
//! against a database in `src/api/upload.rs`. A router line put back to a
//! directory service keeps every one of those tests green while every file on
//! the volume is served again, which is what this file is for.

// A panic is how a test reports failure.
#![allow(clippy::panic, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

const MAIN: &str = "src/main.rs";
const UPLOAD: &str = "src/api/upload.rs";
const SPEC: &str = "specs/turbobaby/upload_media.t27";

fn source(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
        .replace("\r\n", "\n")
}

/// The source with every `//` comment removed.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn spec_value(name: &str) -> String {
    let text = source(SPEC);
    let needle = format!("pub const {name} :");
    let line = text
        .lines()
        .find(|l| l.starts_with(&needle))
        .unwrap_or_else(|| panic!("{SPEC} declares no {name}"));
    line.split_once('=')
        .map(|(_, v)| {
            v.trim()
                .trim_end_matches(';')
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .unwrap_or_else(|| panic!("{SPEC}: {name} has no value"))
}

#[test]
fn the_router_nests_the_reference_gated_read_and_no_directory_service() {
    let main = code_of(&source(MAIN));
    assert_eq!(
        main.matches(".nest_service(\"/uploads\", api::upload::served_uploads(db.clone()))")
            .count(),
        1,
        "{MAIN}"
    );
    assert_eq!(
        main.matches("\"/uploads\"").count(),
        1,
        "one /uploads route"
    );
    assert!(!main.contains("ServeDir::new(\"/data/uploads\")"), "{MAIN}");
    // The folder is named once, beside the writer that fills it.
    let upload = code_of(&source(UPLOAD));
    assert!(upload.contains("pub(crate) const LOCAL_UPLOAD_DIR: &str = \"/data/uploads\";"));
    assert!(
        !main.contains("/data/uploads"),
        "{MAIN} names the folder itself"
    );
}

#[test]
fn a_name_is_checked_then_looked_up_then_served() {
    let upload = code_of(&source(UPLOAD));
    // The folder's directory service sits behind the gate, and only there.
    let start = upload
        .find("pub(crate) fn served_uploads_from(")
        .expect("the read service");
    let service = &upload[start..start + upload[start..].find("\n}\n").expect("it ends")];
    let folder = service
        .find(".fallback_service(tower_http::services::ServeDir::new(dir))")
        .expect("the folder is served");
    let gate = service
        .find(".layer(axum::middleware::from_fn_with_state(")
        .expect("behind a gate");
    assert!(folder < gate, "the gate wraps the folder: {service}");
    assert!(service.contains("only_referenced_uploads"), "{service}");
    assert_eq!(upload.matches("ServeDir::new(").count(), 1, "{UPLOAD}");
    // The gate: the name rule, then the reference, and only then the folder.
    let start = upload
        .find("async fn only_referenced_uploads(")
        .expect("the gate");
    let body = &upload[start..start + upload[start..].find("\n}\n").expect("it ends")];
    let checked = body
        .find("if !is_servable_upload_name(name) {")
        .expect("the name rule");
    let looked_up = body
        .find("match rental_references_upload(&db.orm, name).await {")
        .expect("the reference lookup");
    let served = body
        .find("Ok(true) => next.run(request).await,")
        .expect("a referenced name reaches the folder");
    let refused = body
        .find("Ok(false) => StatusCode::NOT_FOUND.into_response(),")
        .expect("an unreferenced name is not found");
    assert!(
        checked < looked_up && looked_up < served && looked_up < refused,
        "{body}"
    );
    assert_eq!(body.matches("next.run(").count(), 1, "{body}");
    assert!(upload
        .contains("\"SELECT EXISTS (SELECT 1 FROM bikes WHERE image_url = $1) AS referenced\""));
    assert!(upload.contains("format!(\"/uploads/{name}\")"));
    // Nothing on the read path deletes or writes.
    for gone in [
        "remove_file",
        "remove_dir",
        "fs::write",
        "DELETE",
        "UPDATE",
        "INSERT",
    ] {
        assert!(!body.contains(gone), "the read path calls {gone}");
    }
}

#[test]
fn the_contract_and_the_owners_words_are_recorded() {
    assert_eq!(spec_value("OWNER_MEDIA_ANSWER_AT"), "2026-09-26");
    assert_eq!(spec_value("LOCAL_READ_REQUIRES_A_RENTAL_REFERENCE"), "true");
    assert_eq!(spec_value("LOCAL_READ_REFERENCE_COLUMN"), "bikes.image_url");
    assert_eq!(spec_value("MEDIA_ANSWER_DELETES_A_FILE"), "false");
    assert_eq!(
        spec_value("OBJECT_STORE_NEEDS_A_PRODUCTION_LISTING"),
        "true"
    );
    assert_eq!(spec_value("KEY_IS_THE_WHOLE_READ_PERMISSION"), "false");
    assert!(
        source(UPLOAD).contains("«Зачем они вообще нужны мне?»"),
        "{UPLOAD}"
    );
}
