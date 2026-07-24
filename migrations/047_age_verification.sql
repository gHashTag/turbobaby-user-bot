-- Cycle #next: age-verification flag for cannabis compliance.
-- date_of_birth stores the DOB the user supplied; age_verified is the
-- outcome of a server-side age check (>= 20 years for Thailand).

ALTER TABLE loyalty_profiles
ADD COLUMN IF NOT EXISTS age_verified BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS date_of_birth DATE;
