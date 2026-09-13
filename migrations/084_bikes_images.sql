-- Catalog images for the 14 families, one UPDATE each.
--
-- Source: official manufacturer product pages (Honda UK/TH/PH, Yamaha EU/TH,
-- Kawasaki EU), re-hosted in the shop's own media bucket under `bikes/` so
-- the shop does not hotlink a marketing site that may move or 404 the asset.
-- Images are normalised to max 1600px JPEG (click-125 stays PNG: it carries
-- an alpha channel) with metadata stripped.
--
-- D14: these are press/catalog images of the MODEL, not of any unit the fleet
-- holds. No plate number, no renter, no per-unit identifier appears in or is
-- derivable from them — which is exactly why model photos are used instead of
-- photographs of the fleet's own machines.
--
-- xmax-300 and xmax-300-new carry deliberately different images: the 2022
-- pre-facelift asset for the former, the 2023+ facelift for the latter, so
-- the two catalog rows are visually distinguishable products.
--
-- 082 intentionally ships no image_url (its header explains why); this
-- migration adds them after the fact so the seed file stays a tariff register
-- and the image set can be replaced without re-deriving the fleet numbers.

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/click-125.png',
    updated_at = now()
WHERE key = 'click-125';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/nmax-155.jpg',
    updated_at = now()
WHERE key = 'nmax-155';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/xsr-155.jpg',
    updated_at = now()
WHERE key = 'xsr-155';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/cb-300r.jpg',
    updated_at = now()
WHERE key = 'cb-300r';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/forza-300.jpg',
    updated_at = now()
WHERE key = 'forza-300';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/mt-03-300.jpg',
    updated_at = now()
WHERE key = 'mt-03-300';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/xmax-300.jpg',
    updated_at = now()
WHERE key = 'xmax-300';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/xmax-300-new.jpg',
    updated_at = now()
WHERE key = 'xmax-300-new';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/adv-350.jpg',
    updated_at = now()
WHERE key = 'adv-350';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/ninja-400.jpg',
    updated_at = now()
WHERE key = 'ninja-400';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/cb-650r.jpg',
    updated_at = now()
WHERE key = 'cb-650r';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/cbr-650r.jpg',
    updated_at = now()
WHERE key = 'cbr-650r';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/vulcan-s-650.jpg',
    updated_at = now()
WHERE key = 'vulcan-s-650';

UPDATE bikes SET
    image_url = 'https://bucket-production-0ae7.up.railway.app/media/bikes/xadv-750.jpg',
    updated_at = now()
WHERE key = 'xadv-750';
