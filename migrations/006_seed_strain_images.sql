-- Migration 006: Seed image_url for all strains
-- Maps each strain name to its migrated asset path under /assets/

UPDATE strains SET image_url = '/assets/Banana-fritter.webp' WHERE name = 'Banana Fritter';
UPDATE strains SET image_url = '/assets/Black-Mamba.webp'    WHERE name = 'Black Mamba';
UPDATE strains SET image_url = '/assets/Colt-45.webp'        WHERE name = 'Colt 45';
UPDATE strains SET image_url = '/assets/Dipz.webp'           WHERE name = 'Dipz';
UPDATE strains SET image_url = '/assets/Joker-candy.jpeg'    WHERE name = 'Joker Candy';
UPDATE strains SET image_url = '/assets/LA-Ultra.webp'       WHERE name = 'LA Ultra';
UPDATE strains SET image_url = '/assets/Mac-1.jpeg'          WHERE name = 'Mac 1';
UPDATE strains SET image_url = '/assets/Miami-Vice.webp'     WHERE name = 'Miami Vice';
UPDATE strains SET image_url = '/assets/Milk-Monkey.webp'    WHERE name = 'Milk Monkey';
UPDATE strains SET image_url = '/assets/Super-Boof.webp'     WHERE name = 'Super Boof';
UPDATE strains SET image_url = '/assets/Super-Lemon-Haze.webp' WHERE name = 'Super Lemon Haze';
UPDATE strains SET image_url = '/assets/Super-Runtz.webp'    WHERE name = 'Super Runtz';
