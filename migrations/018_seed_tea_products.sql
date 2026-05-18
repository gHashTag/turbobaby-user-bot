-- Migration 018: seed default tea products.
-- Inserts 6 starter tea items if the table is empty. Idempotent via ON CONFLICT.

INSERT INTO tea_products (id, name, subcategory, description, price, stock, image_url, is_available, name_en, description_en, subcategory_en)
VALUES
  ('tea-green-sencha',    'Зелёный Сенча',     'green',   'Японский классический зелёный чай — свежий травяной вкус.',                      180, 50, '', true, 'Green Sencha',    'Classic Japanese green tea — fresh herbal taste.',           'Green Tea'),
  ('tea-black-assam',     'Чёрный Ассам',      'black',   'Крепкий индийский чёрный чай с солодовыми нотами.',                                 160, 50, '', true, 'Black Assam',     'Strong Indian black tea with malty notes.',                  'Black Tea'),
  ('tea-herbal-chamomile','Ромашковый',        'herbal',  'Расслабляющий травяной настой ромашки — без кофеина.',                              140, 40, '', true, 'Chamomile',       'Relaxing chamomile herbal infusion — caffeine-free.',        'Herbal Tea'),
  ('tea-oolong-tieguanyin','Улун Те Гуань Инь','oolong',  'Полуферментированный китайский улун — цветочный аромат.',                            220, 30, '', true, 'Tie Guan Yin',    'Semi-fermented Chinese oolong — floral aroma.',              'Oolong Tea'),
  ('tea-puer-shu',        'Шу Пуэр',           'puer',    'Выдержанный землистый пуэр — глубокий насыщенный вкус.',                            250, 25, '', true, 'Shu Puer',        'Aged earthy puer — deep, rich flavor.',                      'Pu-er Tea'),
  ('tea-rooibos',         'Ройбуш',            'other',   'Южноафриканский напиток из ройбуша — мягкий, без кофеина.',                          170, 40, '', true, 'Rooibos',         'South African rooibos — gentle, caffeine-free.',             'Other')
ON CONFLICT (id) DO NOTHING;
