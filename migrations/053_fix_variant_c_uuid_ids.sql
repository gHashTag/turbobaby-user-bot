-- Cycle fix: Variant C tables were created with `id UUID` while the SeaORM
-- entities (and the rest of the project) use application-generated
-- VARCHAR(36) string IDs. Convert idempotently so inserts bind correctly.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'strain_reviews'
          AND column_name = 'id'
          AND data_type = 'uuid'
    ) THEN
        ALTER TABLE strain_reviews ALTER COLUMN id TYPE CHARACTER VARYING(36) USING id::text;
    END IF;

    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'lab_certificates'
          AND column_name = 'id'
          AND data_type = 'uuid'
    ) THEN
        ALTER TABLE lab_certificates ALTER COLUMN id TYPE CHARACTER VARYING(36) USING id::text;
    END IF;
END $$;
