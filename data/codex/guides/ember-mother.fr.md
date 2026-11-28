# La Mère des Braises

{{table ember-mother.facts | Santé | {health}, +{extra} par joueur en plus | Coup | 7, toutes les 1,5 s | Braise | 2, et brûlure | Expérience | 500 | Butin | [[EmberHeart]] | Où | Son antre, au niveau de la lave}}

La [[mob:EmberMother]] dort sur une île au milieu d’un lac de lave, dans une caverne tout au fond du monde. Il y en a une dans chaque monde qui a des donjons, et une fois vaincue, elle ne revient jamais. C’est le combat le plus difficile du jeu : viens avec ta meilleure armure et de la résistance au feu.

## Trouver l’antre

Fabrique une [[DepthsCompass]] : son aiguille indique l’antre. Elle tourne dans un monde qui n’en a pas, et quand tu es à moins de 4 blocs du centre de l’antre. En obtenir une te vaut le succès [[page:achievements|Elle pointe vers le bas]].

{{recipe DepthsCompass}}

- L’antre se trouve à quelques centaines de blocs du centre du monde : l’une de ses coordonnées, x ou z, est à entre 320 et 704 blocs de 0. Il est placé sous un terrain élevé.
- Son sol est au niveau de la lave : y -54 dans un nouveau monde (y 10 dans un monde antérieur aux profondeurs). Creuse vers le bas en suivant l’aiguille, ou suis les grottes, qui débouchent sur l’antre là où elles le croisent. Voir [[guide:depths|les profondeurs]] pour ce qui t’attend en chemin.
- À moins de 40 blocs, l’antre apparaît sur ta carte. Y entrer te vaut [[page:achievements|À sa porte]].

## L’antre

- Une caverne ronde de 30 blocs de rayon et 16 de haut, dont les parois ondulent jusqu’à 36 blocs.
- Au milieu, un lac de lave d’environ 12 blocs de rayon, avec une île de 4 blocs où elle dort. Une chaussée de 3 blocs de large traverse le lac depuis deux côtés opposés (est et ouest).
- Le sol autour du lac est d’obsidienne et d’ardoise.
- Huit braseros, un [[Campfire|feu de camp]] sur un pilier de [[HewnStone|pierre taillée]], forment un anneau à 25 blocs du centre.
- Aucune créature n’apparaît à l’intérieur, hormis les Escarbilles qu’elle appelle.

## Réveil et sommeil

- Elle se réveille quand un joueur en survie entre dans l’antre. Les joueurs en créatif ne la réveillent pas, et elle ne se réveille jamais en mode Paisible.
- Endormie, rien ne peut la blesser.
- Si personne n’est à l’intérieur pendant **5 secondes**, elle se rendort, **entièrement soignée**, et ses Escarbilles, ses braises et ses feux disparaissent. Partir te soigner te fait perdre le combat.
- Sa barre de santé s’affiche pour tous les joueurs dans l’antre ou à moins de 6 blocs de celui-ci.

### Santé et joueurs

{{table ember-mother.health | Elle a {health} points de santé, plus {extra} par joueur au-delà du premier, en comptant le plus grand nombre de joueurs présents en même temps depuis son réveil. Plus de joueurs lui permettent aussi d’appeler plus d’Escarbilles.}}

{{table ember-mother.scaling | Joueurs | Santé | Escarbilles en même temps | {players} ou plus | {health} +{extra} chacun}}

## Phase 1 : le siège

Tant qu’il lui reste plus de la moitié de sa santé, elle combat de loin, par cycles de 15 secondes.

- Pendant 9 secondes, elle plane sur la rive, hors du lac, à environ 9 blocs du joueur le plus proche. Toutes les 2,25 secondes, elle lance une volée de **3 braises** : une sur toi, deux à moins de 1,5 bloc de toi.
- Une braise inflige **2** (en Normal), t’enflamme pendant 3 secondes, puis te donne Brûlure I pendant 6 secondes. Un bouclier levé dans sa direction arrête tout.
- Pendant les **6 dernières secondes** de chaque cycle, elle descend au sol et se repose. C’est ta fenêtre pour les coups au corps à corps.
- Toutes les 8 secondes, elle appelle une [[mob:Cinderling|Escarbille]] hors du lac, sauf si assez d’entre elles sont déjà sorties (voir le tableau ci-dessus).

### Escarbilles

Elles flottent à 2 blocs du sol, se tiennent à environ 6 blocs et lancent leurs propres braises. Elles ont 10 points de santé. Toucher l’eau les tue sur-le-champ. Tue-les entre les volées de la Mère, sinon elles s’accumulent pendant que tu te bats.

## Phase 2 : la fureur

Sous la moitié de sa santé, elle cesse d’appeler des Escarbilles et fonce sur toi.

- Elle charge à 3,8 blocs par seconde et te suit jusqu’à 10 blocs au-dessus du sol, donc les piliers ne te sauvent pas.
- À portée (2,4 blocs), elle frappe pour **7** avec un fort recul, toutes les 1,5 secondes. Son coup abat un bouclier levé pendant 5 secondes.
- Toutes les 2,5 secondes, elle met le feu à jusqu’à 10 cases du sol, entre 1,5 et 5 blocs autour d’elle. Au plus 40 de ses feux brûlent en même temps ; les plus anciens s’éteignent.
- Si tu es à plus de 6 blocs, elle te lance quand même des volées de braises.

### En tout temps

- Les coups ne la repoussent pas.
- Elle détruit tous les blocs sur son chemin au-dessus du sol, sauf les coffres, tonneaux, fours, alambics et la roche-mère. Impossible de lui barrer la route.
- Elle reste à moins de 24 blocs du centre de l’antre.

### Dégâts selon la difficulté

| Attaque | Facile | Normal | Difficile |
|---|---:|---:|---:|
| Son coup | 4 | 7 | 10 |
| Sa braise | 2 | 2 | 3 |

L’armure retire sa part des deux. Voir [[guide:combat|le combat]].

## Comment la combattre

### Quoi emporter

- **Armure :** une armure complète en diamant ([[DiamondChestplate]] et le reste) absorbe 76 % de chaque coup ; en fer, 60 %. Voir [[page:armour|l’armure]].
- **[[FireResistancePotion|Potions de résistance au feu]] :** elles t’immunisent contre la lave, le feu et la brûlure de ses braises (pas contre les 2 du coup lui-même). Une dure 180 secondes, 360 si elle est brassée avec de la [[Moss|mousse]]. Emportes-en plusieurs. Voir [[page:alchemy|l’alchimie]].
- **[[HealingPotion|Potions de soin]]** et [[RegenerationPotion|potions de régénération]], car tu ne peux pas partir te soigner sans qu’elle se soigne aussi.
- **De la nourriture** qui cale, comme du [[Stew]] ou du [[RoastBeef]] : la santé ne revient d’elle-même qu’avec une jauge de faim presque pleine.
- **Un [[Bow|arc]] et beaucoup de [[Arrow|flèches]]**, pour la première phase, quand elle se tient à distance.
- **Un [[Shield|bouclier]]** pour arrêter les braises et ses coups ; chaque coup qu’il arrête l’abat pendant 5 secondes.
- **Ta meilleure épée**, comme une [[DiamondSword]], avec Tranchant si possible, et une [[StrengthPotion]] pour la fureur.
- **Un [[WaterBucket]]** : il tue une Escarbille qu’il touche, et l’eau versée sur de la lave immobile la change en obsidienne.
- **Un [[Bedroll]]** posé près de l’antre, pour revenir à proximité si tu meurs. Tes objets tombent là où tu es mort et disparaissent au bout de 5 minutes.

### Tactique

1. Bois de la résistance au feu avant d’entrer, et reste sur la rive, hors de la chaussée.
2. En première phase, lève ton bouclier face à ses volées et tire-lui dessus à l’arc.
3. Quand elle se pose pour se reposer, fonce et frappe jusqu’à ce qu’elle décolle.
4. Tue chaque Escarbille dès qu’elle arrive, avant que la suivante ne la rejoigne.
5. Pendant la fureur, éloigne-toi sans cesse de ses feux, frappe après chacun de ses coups, et bois du soin tôt. On ne la distance pas longtemps : finis-en.

## Récompenses

- Elle laisse le [[EmberHeart]], un bloc qui émet une lumière de 12, et une orbe de **500 points d’expérience**.
- Toute la lave de l’antre se change en [[Obsidian]].
- Ses Escarbilles, ses braises et ses feux disparaissent.
- Chaque joueur dans l’antre voit un message de victoire et obtient [[page:achievements|La dernière braise]].
- Elle ne revient jamais dans ce monde.
