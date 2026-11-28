# Santé et faim

En survie, tu as **10 cœurs** (20 points de santé) et **10 pilons** de nourriture (20 points). C’est la nourriture qui te rend de la santé, et tout ce que tu fais l’use peu à peu. En créatif, tu ne subis aucun dégât et tu n’as jamais faim.

## Santé

La santé revient toute seule, **1 point toutes les 4 secondes**, mais seulement tant que ta nourriture est à **18 ou plus** (9 pilons pleins). Chaque point soigné ainsi coûte 6 d’épuisement : soigner est donc ce qui vide le plus vite une barre de nourriture pleine. Sous 18 de nourriture, seuls les potions et les effets de nourriture te soignent.

- Une [[HealingPotion|potion de soin]] rend 4 points d’un coup par niveau ; une [[RegenerationPotion|potion de régénération]] soigne dans la durée.
- La [[FishChowder|chaudrée de poisson]] donne [[effect:Regeneration|Régénération]] II pendant 10 s, le [[GrilledGlowfin|luisant grillé]] [[effect:Regeneration|Régénération]] I pendant 8 s. Voir les [[page:effects|effets de statut]].

### Invulnérabilité

Après un coup, tu ignores les suivants pendant **0,5 seconde**. Le vide, la famine et le poison n’attendent pas : ils te blessent même alors.

## Faim

La faim a deux parts. La **nourriture** est la rangée de pilons que tu vois. La **saturation** est une réserve cachée qui s’y ajoute, dépensée en premier. Tu commences, et tu réapparais, avec la nourriture pleine et 5 de saturation.

Tes efforts s’additionnent en **épuisement**. Chaque tranche de 4 d’épuisement prend 1 point de saturation, ou 1 point de nourriture une fois la saturation épuisée.

| Ce que tu fais | Épuisement |
|---|---:|
| Sprinter au sol | 0,1 par bloc |
| Se déplacer dans l’eau ou la lave | 0,01 par bloc |
| Sauter | 0,05 |
| Sauter en sprintant | 0,2 |
| Casser un bloc | 0,005 |
| Un coup qui touche une créature | 0,1 |
| Subir des dégâts | 0,1 par coup |
| Soigner 1 point de santé | 6 |

Marcher ne coûte rien, voler non plus. Sprinter sur 40 blocs coûte un point de saturation ou de nourriture.

### Sprint et famine

- Il faut **7 de nourriture ou plus** pour sprinter : à 6 ou moins (3 pilons), c’est impossible.
- Avec **plus aucune nourriture**, tu meurs de faim : 1 dégât toutes les 4 secondes. La famine **peut te tuer**.

## Manger

Tiens la nourriture en main et appuie sur le bouton d’utilisation (clic droit). Manger est instantané : un objet par utilisation.

- Tu ne peux pas manger avec une barre de nourriture pleine, sauf si l’aliment donne aussi un effet.
- La nourriture monte jusqu’à 20. La saturation monte aussi, mais jamais au-dessus de ta nourriture : mange quand ta barre est basse pour tout garder.
- Les soupes et ragoûts comme la [[MushroomSoup|soupe de champignons]] te rendent leur [[Bowl|bol]].
- Un [[MilkBucket|seau de lait]] efface tous les effets de statut et te rend le [[Bucket|seau]]. On ne peut pas le boire quand on n’a aucun effet.

Ce que rend chaque aliment est sur la [[page:food|page de la nourriture]].

## Chutes

Une chute de 3 blocs ou moins ne fait aucun mal. Au-delà, tu subis **1 point par bloc**, arrondi au bloc le plus proche : une chute de 23 blocs te tue donc en pleine santé. L’armure n’amortit pas un atterrissage.

{{table health.falls | Chute (blocs) | Dégâts | Cœurs | Les 10}}

- Atterrir dans l’eau, grimper à une [[Ladder|échelle]] ou voler annule la chute.
- Monter un [[mob:Horse|cheval]] retire 3 blocs à ta chute.
- Une chute mortelle dans les 3 secondes qui suivent un coup d’une créature est mise sur le compte de cette créature : « vous a fait tomber dans le vide ».

## Air et noyade

Tu retiens ton souffle pendant **15 secondes**, affichées en 10 bulles de 1,5 s chacune. L’air ne s’épuise que lorsque tes *yeux* sont dans l’eau ou la lave, et revient en 2,5 s dès que tu respires de nouveau. Sans air, tu subis **2 dégâts par seconde**.

Une [[WaterBreathingPotion|potion de respiration aquatique]] empêche l’air de s’épuiser. S’accroupir abaisse tes yeux : cela peut mettre ta tête sous une surface au-dessus de laquelle tu respirerais sinon.

## Feu, lave et magma

| Source | Dégâts | Brûlure ensuite |
|---|---|---|
| Toucher du feu | 1 tout de suite, puis 1 par seconde tant que tu brûles | Au moins 4 s |
| Lave | 4 tout de suite, puis 4 toutes les 0,5 s tant que tu y es | Au moins 15 s |
| [[Magma]] | Comme le feu, tant que tu t’y tiens sans t’accroupir | Au moins 4 s |

- La lave te ralentit aussi à 30 % de ta vitesse.
- Toucher l’eau éteint le feu, et la pluie sur ta tête à ciel ouvert aussi. Ni l’un ni l’autre ne marche tant que tu es encore dans la lave.
- L’armure réduit bien les dégâts de feu et de lave.
- Une [[FireResistancePotion|potion de résistance au feu]] te rend insensible au feu, à la lave et au magma.

## Le vide

Tomber sous **Y -114**, 50 blocs sous le fond du monde, te tue sur-le-champ en survie, quelles que soient ta santé et ton armure. En créatif, tu es simplement ramené au point d’apparition du monde.

## Mort

L’écran de mort dit ce qui t’a tué :

| Cause | Message |
|---|---|
| Une chute | Vous avez heurté le sol trop fort |
| Une chute dans les 3 s qui suivent un coup d’une créature | … vous a fait tomber dans le vide |
| Une créature | … vous a eu |
| Une flèche | … vous a abattu d’une flèche, ou Une flèche vous a eu |
| Le feu ou une brûlure | Vous êtes parti en fumée |
| La lave | Vous avez tenté de nager dans la lave |
| Plus d’air | Vous avez manqué d’air |
| Plus de nourriture | Vous êtes mort de faim |
| Le vide | Vous êtes tombé hors du monde |

Le poison seul ne tue jamais : il s’arrête à 1 point de santé.

### Ce que tu lâches

- Tout ce que tu portais : chaque emplacement de l’inventaire, ton armure, la grille de fabrication et la pile sur ton curseur. Cela reste où tu es mort pendant **5 minutes**, puis disparaît.
- Toute ton expérience est perdue. Une orbe valant **la moitié** de ton expérience, au plus 400 points, reste là où tu es mort.
- Le lieu de ta mort est marqué comme un point de repère sur ta carte.
- En créatif, rien n’est lâché.

## Réapparition et couchage

Tu reviens avec la santé pleine, la nourriture pleine, 5 de saturation et l’air plein, sans brûler et sans aucun effet de statut.

Tu réapparais à ton [[Bedroll|couchage]] si tu en as utilisé un, sinon au point d’apparition du monde. Utiliser un couchage, de jour comme de nuit, en fait ton point de réapparition (« Vous reviendrez ici »). Si le couchage a été cassé depuis, tu retournes au point d’apparition du monde.

La nuit, utiliser un couchage t’endort aussi. Quand tous les joueurs en ligne ont dormi cette nuit-là, le temps saute jusqu’au lever du soleil. Une nuit passée à dormir ne compte pas pour le succès « Toujours là à l’aube ».

## Conseils

- Garde ta nourriture à 18 ou plus avant un combat, sinon tu ne te soigneras pas entre les coups.
- La viande rôtie rassasie bien plus que la viande crue : fais-la cuire dans un [[page:smelting|four]].
- Verse de l’eau le long d’une longue chute, ou descends par une échelle, plutôt que de sauter. Voir aussi le [[guide:combat|combat]] pour l’armure et [[guide:depths|les profondeurs]] pour les lacs de lave.
