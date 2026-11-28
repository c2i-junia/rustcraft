# Progression

Rustcraft n’a pas de niveaux à farmer : tu deviens plus fort en fabriquant de meilleurs outils et de meilleures armures, et les meilleurs matériaux se trouvent plus bas. L’histoire se termine au fond du monde, dans l’antre de la [[mob:EmberMother|Mère des Braises]]. Cette page est la carte de cette route ; chaque étape renvoie au guide qui la couvre.

## Paliers d’outils

Chaque outil existe dans chaque matériau. Une pioche ne peut miner un minerai que si son palier est assez élevé : miné avec une pioche inférieure, le bloc se casse et ne donne rien. La vitesse indique combien de fois plus vite que ta main nue elle casse les blocs pour lesquels elle est faite.

{{table progression.tools | Palier | Fait en | La pioche mine | Vitesse | Durabilité | Dégâts de l’épée | Pierre, charbon, cuivre | + étain, fer | + or, diamant | + obsidienne | N’importe quelles planches | N’importe quelle pierre brute}}

L’or est à part : l’outil le plus rapide de tous, mais il s’use au bout de 32 utilisations et ne mine que ce que mine le bois. Le bronze est une étape entre la pierre et le fer : il mine les mêmes minerais que la pierre, mais dure plus longtemps et creuse plus vite. Chaque pioche mine aussi tout ce que minent les paliers en dessous.

Voir [[page:tools|Outils]] et [[page:weapons|Armes]] pour chaque objet, et [[guide:ores]] pour savoir où se trouve chaque minerai.

## Paliers d’armure

Chaque point d’armure retire 4 % aux coups des créatures, aux flèches, au feu et à la lave. Les chutes, la noyade, la faim et le poison passent à travers. Chaque pièce s’use de 1 chaque fois qu’elle amortit un coup.

{{table progression.armour | Matériau | Casque | Plastron | Jambières | Bottes | Tenue complète | Coups absorbés | Cuir | Bronze | Fer | Diamant | {points} pt, {durability} | {points} pts, {durability}}}

Chaque case donne les points d’armure et la durabilité de la pièce. Le [[Leather|cuir]] vient des vaches. Il n’y a pas d’armure en or. Voir [[page:armour|Armures]].

## La route, étape par étape

### 1. Bois et pierre

Ton premier jour : des bûches, un établi, une pioche en bois, puis des outils en pierre et un [[Furnace|four]]. Un four fond un objet toutes les 10 secondes et c’est le début de tout métal. Suis [[guide:first-day|Ton premier jour]].

### 2. Bronze

Le [[CopperOre|minerai de cuivre]] est courant de la hauteur 96 vers le bas, et n’importe quelle pioche le mine. Le [[TinOre|minerai d’étain]] demande une pioche en pierre. Fonds les deux, puis mélange 3 lingots de cuivre et 1 lingot d’étain en 4 lingots de bronze, dans la grille de l’inventaire.

{{recipe BronzeIngot}}

Le bronze donne les premiers outils et armures en métal.

### 3. Fer

Le [[IronOre|minerai de fer]] demande une pioche en pierre ou en bronze et se trouve de la hauteur 72 à -24. Le fer ouvre l’accès au seau, au bouclier, à la lanterne et à l’[[Anvil|enclume]], et une pioche en fer mine l’or et le diamant.

### 4. Vers les profondeurs

Le sous-sol est divisé en trois paliers. Chaque palier plus bas rend les hostiles plus forts : plus de santé, des coups plus durs, plus nombreux, et un meilleur butin. La première fois que tu atteins un palier, son nom s’affiche à l’écran.

| Palier | Sous la hauteur | Force des hostiles | On y trouve |
|---|---:|---:|---|
| Les Cavernes | 50 | 100 % | Charbon, cuivre, étain, fer ; des cryptes |
| Les Profondeurs | 0 | 150 % | Sombreroche, or, les biomes de grottes profondes, des Escarbilles |
| La Fournaise | -40 | 200 % | Diamant, lacs de lave à partir de -54, sanctuaires, l’antre |

Un palier ne compte que s’il y a de la roche au-dessus de ta tête : le fond d’une mer profonde n’est pas du sous-sol. Dans un monde créé avant l’existence du sous-sol profond, les Profondeurs commencent sous 20 et la Fournaise sous 12. Voir [[guide:depths]].

### 5. Or et diamant

Le [[GoldOre|minerai d’or]] se trouve de -8 à -60, surtout sous -32. Le [[DiamondOre|minerai de diamant]] se trouve sous 16 mais surtout sous -42, dans le palier de la Fournaise. Les deux demandent une pioche en fer. Le diamant fait les meilleurs outils et armures, et avec l’or, la boussole des profondeurs.

### 6. Enchantement

La table de gravure demande un diamant et de l’[[Obsidian|obsidienne]], que seule une pioche en diamant mine. L’obsidienne se forme là où l’eau rencontre une source de lave.

{{recipe EngravingTable}}

Les enchantements coûtent des niveaux d’expérience : 3 pour le niveau I, 5 de plus pour le II, 8 de plus pour le III. Voir [[page:enchanting|Enchantement]].

### 7. Alchimie

L’alambic prépare des potions : soin, vision nocturne, respiration aquatique, résistance au feu, rapidité, force, régénération, et des potions jetables à lancer. La résistance au feu est celle à emporter dans le palier de la Fournaise.

{{recipe Alembic}}

Voir [[page:alchemy|Alchimie]].

### 8. La boussole des profondeurs

Quatre lingots d’or autour d’un diamant. Elle indique l’antre de la Mère des Braises, l’unique antre du monde, et tourne sur elle-même quand tu te trouves en son centre.

{{recipe DepthsCompass}}

### 9. L’antre

L’antre est une grande caverne au niveau de la lave, où la Mère des Braises dort sur une île au milieu d’un lac de lave. Elle s’éveille quand tu entres, a 200 points de santé (80 de plus par joueur supplémentaire), lance des braises, invoque des [[mob:Cinderling|Escarbilles]], et charge une fois descendue à la moitié. Vaincue, elle laisse le Cœur de braise et change la lave de l’antre en obsidienne. Prépare-toi avec un équipement en diamant et de la résistance au feu : voir [[guide:ember-mother]].

## En chemin

- [[guide:structures]] : les villages, les ruines, les cryptes et les sanctuaires renferment des métaux, des outils et des diamants.
- [[guide:trading]] : les villageois vendent des outils et des armures en fer contre des [[Coin|pièces]].
- [[guide:farming]] et [[guide:animals]] : de la nourriture, du cuir et de la laine à volonté.
- [[guide:combat]] : arcs, boucliers, coups critiques.

## Succès

Les 21 succès suivent la même route, et le jeu les liste dans cet ordre. Ils ne s’obtiennent qu’en survie ; ouvre-les depuis le menu du jeu (Échap), sous Progression. Voir [[page:achievements|Succès]].

{{table progression.achievements | **{title}** : {goal}}}

Une nuit dormie ne compte pas pour *Toujours là à l’aube* : tu dois être en vie du début de la nuit à l’aube. *La dernière braise* revient à chaque joueur présent dans l’antre quand la Mère des Braises tombe.
