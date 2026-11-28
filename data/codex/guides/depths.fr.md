# Les profondeurs

Le sous-sol est découpé en trois paliers, chacun plus dur que celui du dessus. Plus tu descends, plus les hostiles sont forts et nombreux, et meilleur est leur butin. La première fois que tu atteins un palier, son nom s’affiche à l’écran et tu gagnes un [[page:achievements|succès]].

## Deux sortes de monde

Un **nouveau monde** est profond : sa pierre cède la place à la sombreroche sous y 0, jusqu’à la bedrock à y -64. Un monde sauvegardé avant l’arrivée des profondeurs reste **peu profond**, tel qu’il a été généré : bedrock à y 0. Les paliers, la lave et les minerais ne sont pas aux mêmes hauteurs dans l’un et l’autre.

| | Nouveau monde | Ancien monde |
|---|---|---|
| Bedrock | y -64, irrégulière jusqu’à -60 | y 0 |
| Grottes les plus basses | y -59 | y 6 |
| Lacs de lave | toute grotte à y -54 et en dessous | toute grotte à y 10 et en dessous |
| [[Gloomrock]] | sous y 0, mêlée à la pierre vers -2 à 0 | aucune |
| Biomes de grotte profonds | y -1 à -56 | aucun |

Les grottes sont creusées sur des étages espacés d’environ 48 blocs. Sous y 48, elles s’ouvrent en salles, et dans un nouveau monde elles s’élargissent en grandes cavernes sous y 0. La bedrock est incassable. Si tu tombes sous y -114, tu quittes le monde et tu meurs.

## Les trois paliers

Un endroit n’est souterrain que s’il y a un vrai sol au-dessus : l’eau, les feuilles et les plantes ne cachent pas le ciel, donc le fond d’un lac n’est dans aucun palier.

| Palier | Nouveau monde | Ancien monde | Force des hostiles |
|---|---|---|---:|
| Les Grottes | sous y 50 | sous y 50 | 100  % |
| Les Profondeurs | sous y 0 | sous y 20 | 150  % |
| La Fournaise | sous y -40 | sous y 12 | 200  % |

### Ce que change la force

Un hostile garde la force du palier où il est apparu. La force multiplie, arrondie au supérieur :

- sa santé, les dégâts de ses coups et l’expérience qu’il donne ;
- le nombre d’hostiles autorisés autour de toi : 10, puis 15, puis 20 dans un rayon de 64 blocs ;
- leur fréquence d’apparition : un essai toutes les 2 secondes dans les Grottes, un plus 50  % de chances d’un deuxième dans les Profondeurs, deux dans la Fournaise ;
- le nombre de tirages de son butin quand tu le tues : un dans les Grottes, un plus 50  % de chances de deux dans les Profondeurs, deux dans la Fournaise ;
- le nombre de gardiens que peut contenir un [[guide:structures|donjon]].

En plus, un hostile tué en profondeur peut lâcher un bonus :

| Palier | Butin bonus |
|---|---|
| Les Grottes | aucun |
| Les Profondeurs | [[IronIngot]], 1 chance sur 8 |
| La Fournaise | [[GoldIngot]], 1 chance sur 6, et [[Diamond]], 1 chance sur 20 |

### Les hostiles des grottes, palier par palier

Santé, dégâts d’un coup en difficulté Normale, et expérience :

{{table depths.hostiles | Hostile | Les Grottes | Les Profondeurs | La Fournaise | {health} PV • {damage} dégâts • {xp} XP}}

Sous un toit et sous y 50, les hostiles habituels sont les [[mob:Gnawer|Rongeurs]] (en meutes de 2 à 4) et les [[mob:Roller|Rouleurs]]. À partir des Profondeurs, une apparition sur 3 est une [[mob:Cinderling|Escarbille]] à la place : elle lance des braises qui t’enflamment.

## Biomes de grotte profonds

Dans un nouveau monde, les grottes entre y -1 et -56 appartiennent à l’un de trois biomes, ou à aucun. Trouve le plus proche avec /locate lush_caves, /locate crystal_caves ou /locate basalt_galleries (voir [[guide:commands|les commandes]]).

### Grottes luxuriantes

Des grottes humides tapissées de [[Moss|mousse]] au sol et au plafond, avec des plaques d’[[Clay|argile]] sur les parois, de petites mares dans le sol et des [[CaveVine|lianes des grottes]] longues de 4 blocs au plus, pendues aux plafonds. Des [[mob:Mossback|Moussecailles]] paisibles y broutent ; elles apparaissent avec la grotte et lâchent de la mousse et parfois du cuir. La mousse allonge la durée des potions à l’[[page:alchemy|alambic]], et la liane des grottes sert à brasser une potion de poison à lancer.

### Grottes de cristal

Des cavernes bordées de [[Geodestone|pierre de géode]], constellées de [[Glowcrystal|cristaux lumineux]], avec des [[CrystalCluster|amas de cristaux]] qui poussent au sol. Les deux se cassent avec n’importe quoi et donnent des [[CrystalShard|éclats de cristal]]. Des [[mob:Shardling|Éclatins]] gardent ces grottes : certains sont là quand on trouve la grotte, et d’autres apparaissent dans le noir sur la pierre de géode. L’éclat des cristaux empêche les hostiles d’apparaître près d’eux.

### Galeries de basalte

Seulement sous y -18, près de la lave : des galeries de [[Basalt|basalte]], avec du [[Magma|magma]] sur certains sols et de la lave dans certaines fosses. Des [[mob:Slagcrawler|Rampe-scories]] apparaissent dans le noir sur le basalte et le magma ; leur morsure brûle. Se tenir sur du magma te brûle, sauf si tu t’accroupis. Le magma est l’ingrédient de la potion de résistance au feu.

## Dangers

- **La lave** inflige 4 dégâts d’un coup et 4 de plus toutes les demi-secondes que tu y restes, te ralentit jusqu’à ramper, et t’enflamme pendant 15 s une fois sorti. L’eau éteint les flammes, mais pas tant que tu es encore dans la lave.
- **Le feu** t’enflamme pendant 4 s, à 1 dégât par seconde. Les braises des Escarbilles et les morsures des Rampe-scories brûlent aussi.
- **L’eau** : tu as 15 s d’air, puis tu perds 2 points de santé par seconde.
- **Les chutes** infligent 1 dégât par bloc au-delà de 3, et l’armure ne les amortit pas.

## Comment se préparer

- **De la lumière.** Toute source de lumière à moins de 8 blocs, sur chaque axe, empêche les hostiles d’apparaître : pose une [[Torch|torche]] ou une [[Lantern|lanterne]] en avançant, et tu laisses un terrain sûr derrière toi.
- **Une armure.** Chaque point d’armure retire 4  % aux coups des mobs, aux flèches, au feu et à la lave. Une armure complète en fer en retire 60  %. Voir [[page:armour|l’armure]] et [[guide:combat|le combat]].
- **La résistance au feu.** La [[FireResistancePotion|potion de résistance au feu]] (base neutre et magma) arrête tous les dégâts de feu, de lave et de magma pendant 3 minutes : bois-la avant la Fournaise.
- **La respiration aquatique.** La [[WaterBreathingPotion|potion de respiration aquatique]] (base neutre et [[Kelp|varech]]) garde ton air plein pendant 3 minutes.
- **La vision nocturne.** La [[NightVisionPotion|potion de vision nocturne]] (base neutre et [[RawGlowfin|luisant cru]]) éclaircit le monde pendant 3 minutes.
- **L’eau.** Un [[WaterBucket|seau d’eau]] éteint les flammes. Une source de lave touchée par l’eau devient de l’obsidienne, et la lave qui coule devient de la pierre brute. L’eau tue aussi les Escarbilles.
- **Une pioche qui permet de remonter.** L’or et les diamants demandent une pioche en fer ; voir [[guide:ores|les minerais]].
- **Un point de réapparition.** Utilise un couchage avant de descendre : tu y reviens si tu meurs. Voir [[guide:health|la santé]].

Au niveau de la lave, la Mère des Braises attend dans son antre. Une [[DepthsCompass|boussole des profondeurs]] t’y mène. Voir [[guide:ember-mother|la Mère des Braises]].
