# Combat

Un coup ne fait tous ses dégâts que lorsque ton arme a fini de recharger. Attends entre deux coups, frappe en tombant pour un coup critique, et laisse l’armure et un bouclier encaisser ce que les créatures te rendent. Chaque arme est listée sur la [[page:weapons|page des armes]], chaque pièce d’armure sur la [[page:armour|page des armures]].

## Mêlée

Clic gauche sur une créature à portée pour la frapper. Les dégâts d’un coup se calculent en quatre étapes :

1. **Les dégâts de l’arme** : 1 à mains nues ou avec tout ce qui n’est pas une arme, plus 1 par niveau de Tranchant.
2. **Les effets** : [[effect:Strength|Force]] ajoute 3 par niveau, [[effect:Weakness|Faiblesse]] en retire 2 par niveau, sans jamais descendre sous 1.
3. **La recharge** : un coup complet fait tout, un coup donné aussitôt seulement un cinquième. Entre les deux, elle croît avec le carré de la charge : à moitié rechargée, 40 %.
4. **Un coup critique** multiplie le résultat par 1,5. Tout coup fait au moins 1.

### Armes et recharge

Maintenir le bouton frappe de nouveau dès que l’arme a rechargé. Changer d’objet fait repartir la recharge de zéro. Une main nue recharge en 0,4 s.

{{table combat.melee | Arme | Dégâts | Recharge | Dégâts par seconde | Usure par coup | Durabilité | Aucune | Jamais}}

Les épées font 4, les haches 6 et les autres outils 2, plus 1 pour la pierre ou le bronze, 2 pour le fer et 3 pour le diamant ; le bois et l’or n’ajoutent rien. La pioche représente la pelle et la houe, qui frappent pareil.

### Coups critiques

Un coup rechargé à 90 % au moins pendant que tu **tombes** (en l’air, en descendant, sans voler ni nager) est critique : **dégâts x1,5**. Saute, puis frappe pendant la descente.

### Balayages

Un coup d’épée rechargé à 90 % au moins pendant que tu es **au sol** balaie aussi : toute autre créature à moins de 2 blocs de la cible, et devant toi, subit un tiers des dégâts (au moins 1).

### Portée et recul

- Tu atteins les créatures à **3 blocs** de tes yeux, ou **5 avec une lance**.
- Un coup complet repousse la créature à 6 blocs par seconde, un coup précipité à moitié moins. La masse repousse 2,2 fois plus loin.
- Une créature que tu touches ne peut plus être blessée pendant 0,5 s, et un animal que tu touches s’enfuit pendant 5 s.
- L’aboiement du [[mob:Stumpling|Souchon]] pare les coups de face. Une hache écarte sa garde pendant 4 secondes, et les haches lui infligent 1,5 fois leurs dégâts.

### Usure des armes

Chaque coup qui porte use une épée ou la masse de 1, et tout autre outil de 2 : une hache ou une pioche utilisée comme arme s’use deux fois plus vite. La lance et la hache de jet ne s’usent jamais. Répare les armes usées à l’[[Anvil|enclume]] (voir [[page:enchanting|enchantement et enclume]]).

## Armes à distance

Maintiens le bouton d’utilisation (clic droit) pour bander, relâche-le pour tirer.

### Arc

Un [[Bow|arc]] est tendu à fond après 1 seconde ; relâché avant 0,15 s, il ne tire rien. La flèche vole jusqu’à 42 blocs par seconde et retombe en courbe. Chaque niveau de Puissance de l’arc ajoute 1 dégât. Chaque tir consomme une [[Arrow|flèche]] et use l’arc de 1. Pendant que tu bandes, tu marches à 40 % de ta vitesse.

{{table combat.arrows | Tension | Dégâts de la flèche}}

### Arbalète

Une [[Crossbow|arbalète]] met 1,25 s à se charger et ne tire qu’une fois entièrement chargée. Son [[Bolt|carreau]] fait toujours **11 dégâts** et traverse 2 créatures avant de s’arrêter dans une troisième.

### Hache de jet

La [[ThrowingAxe|hache de jet]] se ramène en arrière jusqu’à 0,6 s puis se lance : 4 dégâts pour un jet rapide, 8 pour un jet complet. Elle tombe au sol telle quelle, pour être ramassée de nouveau. En mêlée, elle frappe pour 5 au rythme d’une hache.

### Potions jetables

La [[SplashPoisonPotion|potion jetable de poison]], la [[SplashSlownessPotion|potion jetable de lenteur]] et la [[SplashWeaknessPotion|potion jetable de faiblesse]] se lancent comme la hache de jet (0,6 s pour un jet complet) et éclatent là où elles atterrissent. Elles ne font aucun dégât elles-mêmes : tous ceux qui sont à moins de 4 blocs reçoivent l’effet, en entier au centre et à moitié au bord. Voir l’[[page:alchemy|alchimie]].

Les flèches, les carreaux et les haches lancées touchent les joueurs comme les créatures, et un bouclier les arrête. En créatif, tirer ne consomme aucune munition.

## Bouclier

Maintiens le bouton d’utilisation pour lever un [[Shield|bouclier]]. Il met 0,25 s à se lever, et tant qu’il est levé tu marches à mi-vitesse et tu ne peux pas sprinter.

- Il arrête tous les coups et tirs venant de ta **moitié avant**, mesurée à plat sur le sol.
- Il s’use des dégâts qu’il arrête (au moins 1 par coup) : 336 points en tout.
- La charge d’un [[mob:Roller|Rouleur]] ou le coup de la [[mob:EmberMother|Mère des Braises]] le fait tomber pendant 5 secondes.
- Il fonctionne depuis **n’importe quel emplacement de la barre rapide** tant que ta main tient une épée, une hache, une lance, la masse ou rien, donc tu peux te battre et parer en même temps.
- On ne peut pas le lever à cheval, en bandant un arc ou en tenant de la nourriture.

## Armure

Chaque point d’armure absorbe **4 %** d’un coup. Chaque niveau de Protection sur n’importe quelle pièce en ajoute 1 % de plus. Les points d’armure comptent jusqu’à 20, et le total ne dépasse jamais 90 %. Ce qui est absorbé sous un point entier est reporté sur le coup suivant, si bien que la réduction est exacte à la longue.

{{table combat.armour | Matériau | Casque | Plastron | Jambières | Bottes | Armure complète | Absorbé | Cuir | Bronze | Fer | Diamant}}

Une armure de diamant complète avec Protection III sur les quatre pièces absorbe 76 + 12 = 88 % d’un coup.

### Durabilité

Chaque coup que l’armure réduit use de 1 chaque pièce que tu portes.

{{table combat.durability | Matériau | Casque | Plastron | Jambières | Bottes | Cuir | Bronze | Fer | Diamant}}

### Ce que l’armure ne réduit pas

L’armure absorbe les coups et les projections des créatures, les flèches, le feu et la lave. Elle ne fait **rien** contre les chutes, le vide, la noyade, la famine ou le poison. Voir [[guide:health|santé et faim]].

## Créatures hostiles et difficulté

La difficulté du monde change la force des coups des hostiles. En **Paisible**, il n’y en a aucun. **Facile** réduit à peu près un coup de moitié (la moitié, plus 1), **Difficile** y ajoute encore la moitié. Sous terre, un hostile né dans les Profondeurs est 1,5 fois plus fort et un hostile né dans la Fournaise deux fois plus fort, avant l’application de la difficulté (voir [[guide:depths|les profondeurs]]).

{{table combat.strikes | Créature | Facile | Normale | Difficile}}

Dégâts d’un coup en surface, en points de santé (2 par cœur), avant l’armure. Chaque créature est décrite sur la [[page:creatures|page des créatures]].

## Expérience

Une créature que tu tues laisse une orbe d’expérience. Seuls les hostiles, le [[mob:Mossback|Moussecaille]] et les poissons en donnent : les autres animaux et les villageois n’en donnent aucune. Les hostiles nés dans les Profondeurs en donnent 1,5 fois plus, dans la Fournaise deux fois plus.

{{table combat.experience | Créature | Expérience}}

Dépense l’expérience à la [[EngravingTable|table de gravure]] et à l’[[Anvil|enclume]].

## Au combat à cheval

Quand tu montes un [[mob:Horse|cheval]], un [[mob:Donkey|âne]] ou un [[mob:Mule|mulet]], ta monture prend la moitié de chaque coup qu’une créature te destine, arrondie au supérieur, et tu subis le reste : un coup de 3 fait 2 à la monture et 1 à toi. On ne peut pas lever un bouclier en selle. Voir [[guide:animals|apprivoiser, élever et monter]].
