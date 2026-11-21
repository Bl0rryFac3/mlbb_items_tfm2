# MLBB Items for Teamfight Manager 2

MLBB Items is an unofficial Windows mod for **Teamfight Manager 2 v0.7.3**. It adapts equipment from Mobile Legends: Bang Bang to Teamfight Manager 2 and adds six-slot builds, custom Mythical upgrades, jungle equipment, and roaming blessings.


## Installation

1. Close the game. Disable Riot Items and remove any older MLBB Items installation from the game's `mods` folder. Back up any presets or personal settings you want to keep before replacing the folder.
2. Extract `mlbb_items_v1.4.zip` into the game's `mods` folder. The expected paths are `mods/mlbb_items/mod.mod_info` and `mods/mlbb_items/mlbb_items.dll`.
3. Start the game, enable **MLBB Items**, and restart the game. Use a separate save when trying the mod for the first time.
4. If the new items are missing from a new career, save, return to the main menu, and reload. Repeat if needed until the **Mods** column shows **Match**.

## Equipment

The catalog includes basic and advanced components, six optional Forged components, 57 finished combat items, seven upgraded boots, and 57 Mythical upgrades. Items such as Golden Staff, Endless Battle, Divine Glaive, Wind of Nature, and Immortality have effects adapted to Teamfight Manager 2. Older equipment such as Bloodlust Axe, Calamity Reaper, and Scarlet Phantom also appears in the catalog.

- **Forged components** provide alternate routes toward a finished item. An upgrade replaces equipment in its existing slot. When Item Info lists multiple starting items, each is an alternate path.
- **Mythical equipment** upgrades finished combat items and uses a violet icon frame. Set **Options → Competition → Item Rules** to allow Mythicals in all slots, only the first slot, or no slots.
- **Jungle equipment** starts with Hunter’s Knife and branches into Star Shard, Raptor Machete, or Beast Killer. Choose a branch in **Builds**, let the game choose with **Auto**, or turn off automatic selection with **None**.
- **Roaming blessings** go on completed boots bought by Supports. Favor, Encourage, Dire Hit, and Conceal share the boot slot. A selected blessing starts working once the boots are equipped, and roaming income strengthens it over the match.

<details>
<summary>Finished combat items and upgraded boots</summary>

**Combat items** · Antique Cuirass, Beast Killer, Berserker's Fury, Blade Armor, Blade of Despair, Blade of the Heptaseas, Blood Wings, Bloodlust Axe, Bloodthirsty King, Brute Force Breastplate, Calamity Reaper, Chastise Pauldron, Clock of Destiny, Concentrated Energy, Corrosion Scythe, Cursed Helmet, Demon Hunter Sword, Divine Glaive, Dominance Ice, Enchanted Talisman, Endless Battle, Feather of Heaven, Flask of the Oasis, Fleeting Time, Genius Wand, Glowing Wand, Golden Staff, Great Dragon Spear, Guardian Helmet, Haas' Claws, Holy Crystal, Hunter Strike, Ice Force, Ice Queen Wand, Immortality, Legacy Windtalker, Lightning Truncheon, Malefic Gun, Malefic Roar, Oracle, Queen's Wings, Radiant Armor, Raptor Machete, Rose Gold Meteor, Scarlet Phantom, Sea Halberd, Shadow Twinblades, Sky Piercer, Star Shard, Starlium Scythe, Thunder Belt, Valkyrie's Shield, War Axe, Wind of Nature, Windtalker, Winter Crown, Wishing Lantern.

**Boots** · Arcane Boots, Demon Shoes, Magic Shoes, Rapid Boots, Swift Boots, Tough Boots, Warrior Boots.

</details>

## Builds and item settings

- **Builds** lets you pin equipment for up to six slots. Smart Builds fills open slots using the champion’s role and damage profile. **Force Jungle First** and **Force Roam Boots First** control where eligible role equipment goes in the match build.
- **Check Tactics** shows both teams’ equipment plans during a match.
- **Statistics → Build Stats** shows item win rates, pick rates, and game counts by champion and role. It also suggests items when enough match data is available.
- **Options → Competition → Item Patcher Settings** lets you adjust supported item stats and queue changes for your save.

Prices, stats, and effects are balanced for Teamfight Manager 2. Effects that rely on a manual item button in MLBB can trigger automatically here. For example, Wind of Nature can activate when the wearer takes damage at low health.

## Repository files

This repository contains the mod’s item data, configuration, and artwork. The Windows package contains the compiled mod.

## Credits

Based on [Riot Games Item Expansion Pack by shirograhm](https://github.com/shirograhm/riot-items-tfm2). The upstream code is available under the [MIT license](LICENSE). Mobile Legends: Bang Bang equipment names and artwork belong to their respective rights holders.
