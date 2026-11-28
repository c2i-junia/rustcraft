# Modes de jeu

Un monde est créé avec un mode de jeu, une difficulté et un type de monde, tous choisis dans la fenêtre **Nouveau monde** de l’écran Solo (Ctrl + N). Le mode de jeu et la difficulté peuvent être changés plus tard avec une commande ; le type de monde, non.

## Survie et créatif

Il y a deux modes de jeu. La survie est le mode par défaut.

|  | Survie | Créatif |
|---|---|---|
| Dégâts | Les chutes, les créatures, le feu, la lave, la noyade et la faim te blessent | Rien ne te blesse |
| Faim et air | La nourriture baisse, l’air manque sous l’eau | Jamais |
| Santé | Ne revient que tant que la nourriture est à 18 ou plus | Revient d’elle-même, 1 point toutes les 4 s |
| Vol | Non | Oui, F l’active ; tu voles à travers les blocs |
| Objets | Minés, fabriqués, trouvés | Pris dans le navigateur d’objets à côté de l’inventaire |
| Enchantement et enclume | Coûtent des niveaux d’expérience | Gratuits |
| Arcs et arbalètes | Utilisent une flèche ou un carreau par tir | N’ont besoin d’aucune munition |
| Effets des créatures | Le poison, la lenteur et d’autres s’appliquent | Jamais ; les potions agissent quand même |
| Mort | Tu laisses tomber tout ce que tu portes | Tu ne laisses rien tomber |
| Succès | Obtenus | Non obtenus ; seul le temps de jeu est compté |

En créatif, le catalogue du navigateur d’objets donne des objets : un clic met une pile pleine sur le curseur, un clic droit un seul objet, et Maj + clic une pile pleine dans ton inventaire. Une pile reposée sur le catalogue est détruite. Passer du créatif à la survie en volant te remet sur tes pieds.

### Changer de mode

Tape **/gamemode creative** ou **/gamemode survival** dans le tchat (T). Cela ne change que ton propre mode. Dans un monde solo, tu peux toujours l’utiliser ; sur un serveur, il faut les droits d’opérateur. Voir [[guide:commands]].

Le mode de chaque joueur est enregistré avec lui. Le mode choisi pour le monde est celui dans lequel démarrent les nouveaux joueurs.

## Difficulté

La difficulté appartient au monde et s’applique à tous ceux qui s’y trouvent. Elle change la force des coups des créatures hostiles, et même leur existence. Normale est la valeur par défaut.

| Difficulté | Ce qu’elle fait |
|---|---|
| Paisible | Aucune créature hostile. Celles déjà dans le monde disparaissent. La Mère des Braises ne s’éveille jamais. |
| Facile | Un coup de *d* inflige la moitié de *d* plus 1, arrondie à l’entier inférieur, et jamais plus que *d*. |
| Normale | Les coups infligent leurs dégâts complets. |
| Difficile | Les coups infligent la moitié en plus, arrondie à l’entier inférieur. |

Ce que deviennent les coups de certains hostiles, en points de santé (tu en as 20) :

{{table game-modes.blows | Créature | Facile | Normale | Difficile}}

Les hostiles frappent aussi plus fort à mesure que tu descends : voir [[guide:depths]]. L’armure réduit ces coups (voir [[page:armour|Armures]]). La difficulté ne change ni les chutes, ni le feu, ni la noyade, ni la faim.

Change-la avec **/difficulty peaceful**, **easy**, **normal** ou **hard**. En paisible, les hostiles disparaissent au prochain tick.

## Types de monde

| Type | À quoi ressemble le monde |
|---|---|
| Normal | Des continents, des mers et tous les biomes à leur taille habituelle. |
| Grands biomes | Des biomes deux fois plus grands, et les continents avec eux. |
| Archipel | Des îles éparpillées sur une vaste mer. |
| Biome unique | Un seul biome partout. |

Voir [[page:biomes|les biomes]] pour chacun d’eux.

La fenêtre Nouveau monde propose Normal, Grands biomes et Archipel. Un monde à biome unique est créé par un serveur dédié, quand il crée son monde :

**rustcraft-server --world-type single-biome --biome desert**

Le biome est désigné par sa clé, comme **plains**, **jungle** ou **ice_spikes**. Les options **--world-type**, **--game-mode** et **--difficulty** du serveur ne s’appliquent qu’à un monde qu’il crée : un monde existant garde les siennes. Voir [[guide:multiplayer]].

Le champ de graine de la fenêtre Nouveau monde accepte un nombre de 0 à 4294967295 ; laissé vide, la graine est aléatoire.

## Voir aussi

- [[guide:first-day|Ton premier jour]] en survie
- [[guide:health]]
- [[guide:controls]]
