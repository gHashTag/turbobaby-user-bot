-- Add default images for tea products and sets
UPDATE tea_products SET image_url = '/assets/tea/placeholder.svg' WHERE image_url = '' OR image_url IS NULL;
UPDATE accessory_sets SET image_url = '/assets/sets/placeholder.svg' WHERE image_url IS NULL;