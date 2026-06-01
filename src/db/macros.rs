/// Cycle #100: observability for schema-drift on `try_get` reads.
///
/// Wraps `QueryResult::try_get` with a fallback default and emits a
/// `tracing::warn!` (target `db.schema_drift`) when the read fails.
///
/// Replaces the common pattern
/// ```ignore
/// let v = r.try_get::<f64>("", "price").unwrap_or(0.0);
/// ```
/// with
/// ```ignore
/// let v: f64 = try_get_warn!(r, "price", 0.0);
/// ```
///
/// Same runtime behaviour (default on Err) but emits a grep-able log
/// line on schema drift (column renamed/dropped, type changed, NOT NULL
/// → NULL). Cycle #98 closed the financial-write hole with fail-closed
/// `Result::collect` (pattern #22); this is the lower-bar "fail-open
/// but log" variant for display-path reads.
///
/// The type is inferred from `$default` and the binding site. Use a
/// typed default (`0.0_f64`, `false`, `0_i32`) when inference is
/// ambiguous.
#[macro_export]
macro_rules! try_get_warn {
    ($row:expr, $col:literal, $default:expr) => {{
        match $row.try_get("", $col) {
            Ok(v) => v,
            Err(e) => {
                ::tracing::warn!(
                    target: "db.schema_drift",
                    column = $col,
                    error = %e,
                    "try_get fell back to default — possible schema drift"
                );
                $default
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    /// Sanity-check the macro expands and selects the Ok branch when
    /// the underlying `try_get` succeeds. A full schema-drift test would
    /// require a live SeaORM `QueryResult`, which is awkward to construct
    /// in isolation — the integration tests cover that path implicitly.
    struct FakeRow;
    impl FakeRow {
        fn try_get<T: Default>(&self, _pre: &str, _col: &str) -> Result<T, &'static str> {
            Ok(T::default())
        }
    }
    struct FailRow;
    impl FailRow {
        fn try_get<T: Default>(&self, _pre: &str, _col: &str) -> Result<T, &'static str> {
            Err("missing column")
        }
    }

    #[test]
    fn macro_returns_ok_value() {
        let r = FakeRow;
        let v: i64 = crate::try_get_warn!(r, "x", 42);
        assert_eq!(v, 0); // FakeRow::try_get returns default(), not 42
    }

    #[test]
    fn macro_returns_default_on_err() {
        let r = FailRow;
        let v: i64 = crate::try_get_warn!(r, "x", 42);
        assert_eq!(v, 42);
    }
}
