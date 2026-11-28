# Les structures

Le monde contient des villages en surface, trois sortes de donjons de la surface jusqu’à la lave, des épaves sur le fond de la mer avec un trésor enterré à proximité, et l’antre de la Mère des Braises. Leurs coffres sont remplis à la génération du monde.

## Comment fonctionne le butin

Chaque coffre et chaque tonneau a 27 emplacements. Chaque ligne de sa table est tirée séparément : d’abord la chance, puis une quantité entre les deux nombres. Un coffre peut contenir beaucoup de lignes ou aucune. Le contenu d’un conteneur est fixé par la graine du monde et sa position.

## Villages

Il y a au plus un village par zone de 512 x 512 blocs, avec une chance de 4 sur 5. Un village a besoin d’une terre sèche et assez plate sous la hauteur 76, bien à l’intérieur d’un biome qui a un style de village, loin des arbres géants de la jungle. Il s’étend à 64 blocs de son centre au plus.

### Styles

{{table structures.styles | Style | Biomes | Matériaux | Plaines | Murs en planches de chêne, poteaux en bûches de chêne, toits de chaume ; rues de gravier | Savane | Murs en pisé, poteaux et sols en acacia, toits de chaume ; rues de gravier | Désert | Murs en pisé, sols en pierre taillée, poteaux et toits en brique ; rues de sable | Enneigé | Planches et bûches d’épicéa, toits d’épicéa ; rues de gravier}}

Les jungles, les montagnes, les pics de glace, les mers et les rivières n’ont pas de villages.

### Bâtiments

Un puits se dresse sur une place au centre. Des rues partent de 2 à 4 de ses côtés, se ramifiant jusqu’à 5 fois, avec un lampadaire tous les 9 blocs. Jusqu’à 14 bâtiments les bordent :

| Bâtiment | Fréquence | Maximum par village |
|---|---:|---:|
| Petite maison | 40 | illimité |
| Champ (terre labourée plantée de seigle) | 15 | 4 |
| Grande maison | 14 | illimité |
| Grenier | 10 | 2 |
| Enclos | 9 | 2 |
| Forge | 7 | 1 |

La « fréquence » est le poids avec lequel chaque bâtiment est tiré, sur 95.

### Enclos

Chaque enclos contient 2 à 4 animaux, choisis parmi ceux du style du village :

- Plaines : [[mob:Sheep|moutons]], [[mob:Pig|cochons]], [[mob:Chicken|poules]], [[mob:Cow|vaches]]
- Savane : [[mob:Cow|vaches]], [[mob:Goat|chèvres]], [[mob:Chicken|poules]]
- Désert : [[mob:Goat|chèvres]], [[mob:Chicken|poules]]
- Enneigé : [[mob:Sheep|moutons]], [[mob:Rabbit|lapins]]

### Villageois

Chaque maison, petite ou grande, a un villageois. Leurs métiers suivent l’ordre dans lequel les maisons ont été construites : [[mob:Farmer|Fermier]], [[mob:Smith|Forgeron]], [[mob:Cartographer|Cartographe]], puis on recommence. Ils se promènent autour de chez eux le jour et rentrent la nuit. Rien ne peut leur faire de mal et les hostiles les ignorent. Le cartographe vend la direction du donjon le plus proche. Voir [[guide:trading|le commerce]].

### Butin des villages : forge

{{table structures.forge | Objet | Chance | Quantité}}

### Butin des villages : grenier

{{table structures.granary | Objet | Chance | Quantité}}

### Butin des villages : maisons et autres bâtiments

{{table structures.house | Objet | Chance | Quantité}}

## Donjons

Chaque zone de 256 x 256 blocs peut contenir un donjon de chaque sorte. Leur butin est meilleur plus ils sont profonds. Les mondes sauvegardés avant l’arrivée des donjons n’en ont aucun.

| Donjon | Chance par zone | Où | Conteneurs | Gardiens |
|---|---:|---|---|---|
| Ruine | 50  % | Sur une terre plate et sèche, sous les montagnes, loin des villages | 3 coffres, 3 tonneaux | 4 [[mob:Bramble|Ronces]], 3 [[mob:Gnawer|Rongeurs]] |
| Crypte | 75  % | Au flanc d’une paroi de grotte, y 4 à 50 (y 20 à 50 dans un ancien monde) | 2 coffres, 1 tonneau | 6 [[mob:Gnawer|Rongeurs]] |
| Sanctuaire | 60  % | Au flanc d’une grotte près de la lave, y -55 à -47 (y 9 à 17 dans un ancien monde) | 3 coffres | 4 [[mob:Roller|Rouleurs]], 2 [[mob:Cinderling|Escarbilles]] |

- **Ruine** : une salle sans toit aux murs brisés. Une [[Ladder|échelle]] descend vers une cave.
- **Crypte** : une nef entre deux rangées de tombes, avec une chambre derrière.
- **Sanctuaire** : une salle de piliers d’[[Obsidian|obsidienne]] entre deux canaux de lave, un autel au fond. Il contient aussi un [[GoldBlock|bloc d’or]] et deux [[BronzeBlock|blocs de bronze]]. Il se trouve dans le [[guide:depths|palier de la Fournaise]], donc ses gardiens sont deux fois plus forts.

La zone qui contient l’antre de la Mère des Braises n’a ni crypte ni sanctuaire.

### Gardiens

Chaque gardien a sa place dans le plan du donjon. Un donjon se réveille tant qu’un joueur est à moins de 36 blocs. Toutes les 2 secondes, il a alors 1 chance sur 4 d’envoyer un gardien à l’une de ses places, davantage dans les paliers plus profonds (multiplié par la force du palier).

- Les gardiens n’ont pas besoin d’obscurité, contrairement aux autres hostiles.
- Un gardien n’apparaît jamais à moins de 8 blocs d’un joueur, ni là où un joueur peut voir la place.
- Un donjon contient au plus 5 hostiles à la fois (moins s’il a moins de places), multiplié par la force du palier.
- Rien n’apparaît en mode Paisible.

**Pour arrêter les gardiens**, pose une [[Torch|torche]], une [[Lantern|lanterne]] ou un [[Campfire|feu de camp]] allumé à moins de 3 blocs de chaque place, ou comble la place avec n’importe quel bloc.

### Butin des ruines

{{table structures.ruin | Objet | Chance | Quantité}}

### Butin des cryptes

{{table structures.crypt | Objet | Chance | Quantité}}

### Butin des sanctuaires

{{table structures.sanctuary | Objet | Chance | Quantité}}

## Épaves et trésors enterrés

Chaque zone de 320 x 320 blocs a 70  % de chances d’avoir une épave, sur un fond marin de 9 à 40 blocs de profondeur. Son coffre, sous le pont de la poupe, contient toujours une [[TreasureMap|carte au trésor]] en plus de son butin.

La carte mène à un coffre enterré à 2 blocs sous le sol, à 24 à 72 blocs de l’épave, sur une rive ou dans une eau peu profonde quand il y en a une près. Tant que tu portes la carte, l’endroit est marqué d’une croix rouge nommée « X » sur tes cartes. Creuse là pour trouver le coffre.

### Coffre d’épave

{{table structures.shipwreck | Objet | Chance | Quantité}}

### Trésor enterré

{{table structures.treasure | Objet | Chance | Quantité}}

## L’antre de la Mère des Braises

Chaque monde avec des donjons a un antre, une grande caverne au niveau de la lave autour d’un lac de lave, où dort le boss du jeu. Une [[DepthsCompass|boussole des profondeurs]] y mène. Voir [[guide:ember-mother|la Mère des Braises]].

## Trouver les structures

Tape /locate suivi d’un nom pour obtenir la plus proche (voir [[guide:commands|les commandes]]) :

| Structure | Commande |
|---|---|
| Village | /locate village |
| Ruine | /locate ruin |
| Crypte | /locate crypt |
| Sanctuaire | /locate sanctuary |
| Épave | /locate shipwreck |

Sans commandes, explore : un village est ajouté à tes points de passage quand tu t’en approches à moins de 48 blocs, un donjon à moins de 8 blocs, l’antre à moins de 40. Le cartographe vend aussi la direction du donjon le plus proche.
