# What survives of the client's strain catalog

Migration `083_drop_cannabis_catalog.sql` dropped `strains`, `strain_reviews` and
`lab_certificates` from the **railway** database at 2026-09-13 11:49:12 UTC.
These are the references that outlived the drop, gathered read-only.

## Named by `strain_of_day_log` (admin promoted these by hand)
```
-pink-oreoz | 💗 Pink Oreoz | 2026-04-12
-black-truffle | 🖤 Black Truffle | 2026-04-12
joker-candy | Joker Candy | 2026-04-14
dipz | DIPZ | 2026-04-14
super-lemon-haze | SUPER LEMON HAZE | 2026-04-15
```

## Named by `user_plants` / `plant_rewards` (customers grew these)
```
banana-fritter  ->  BANANA FRITTER
-black-patronus  ->  🖤 Black Patronus
dipz  ->  DIPZ
mac-1  ->  MAC 1
milk-monkey  ->  MILK MONKEY
-top-subzero-cherry  ->  ❄️ Top Subzero Cherry
-v6-haze  ->  🎧 V6 Haze
```

## Referenced by `sets` (bundles that still point at strain ids)
```
Hash Rockets | 0 | {} | {}
👑 Kiefez Glass Tip Blunt | 0 | {} | {}
Lieges infused Blunts | 0 | {} | {}
🥜 New at WoodyWeedPecker — Snickers Cake! | 0 | {} | {}
🌙 NIGHT PACK | 5 | {2258af62-1df1-4b33-ada3-d07a66f3edeb,811d93b9-2f80-4004-8ae2-39585046a03e,453061b8-31df-4ff3-95e7-33e2db75fd75,2b6db071-972f-4545-a3e9-6ce9532383a8,2706a1a6-8f67-4c44-9551-e201ff3d3dd5} | {-black-truffle,-black-patronus,-death-star-og,-top-subzero-cherry,-black-cherry-runtz}
🎉 PARTY PACK | 5 | {2b6db071-972f-4545-a3e9-6ce9532383a8,6cc19642-4670-4806-8c29-9ba4bb5723e1,2706a1a6-8f67-4c44-9551-e201ff3d3dd5,87bb9443-de39-4adc-b1f9-c3c7349a8f4d,2258af62-1df1-4b33-ada3-d07a66f3edeb} | {}
WOODY’S SOMMELIER SELECTION  Personally Curated Premium Flower | 3 | {7ca37991-d52e-444f-89ce-3efb82c15870,eff225b5-56e1-47f9-b3da-0e6969ec2403,2c5bfa4b-5a55-49c6-8948-43ca618aa517} | {}
WOODY’S SOMMELIER SELECTION  Personally Curated Premium Flower | 3 | {ee6fda06-f7dd-455e-a2b8-dfb71fddb47d,5055817b-7fad-445b-80e4-acee20c850d9,d4f559cb-8081-44c4-aa86-dd06812dc429} | {}
```

## Referenced by `group_fact_posts`
```
```

## Distinct strain ids still referenced anywhere
```
26 distinct ids
2258af62-1df1-4b33-ada3-d07a66f3edeb
2706a1a6-8f67-4c44-9551-e201ff3d3dd5
2b6db071-972f-4545-a3e9-6ce9532383a8
2c5bfa4b-5a55-49c6-8948-43ca618aa517
453061b8-31df-4ff3-95e7-33e2db75fd75
5055817b-7fad-445b-80e4-acee20c850d9
6cc19642-4670-4806-8c29-9ba4bb5723e1
7ca37991-d52e-444f-89ce-3efb82c15870
811d93b9-2f80-4004-8ae2-39585046a03e
87bb9443-de39-4adc-b1f9-c3c7349a8f4d
banana-fritter
-black-cherry-runtz
-black-patronus
-black-truffle
d4f559cb-8081-44c4-aa86-dd06812dc429
-death-star-og
dipz
ee6fda06-f7dd-455e-a2b8-dfb71fddb47d
eff225b5-56e1-47f9-b3da-0e6969ec2403
joker-candy
mac-1
milk-monkey
-pink-oreoz
super-lemon-haze
-top-subzero-cherry
-v6-haze
```
