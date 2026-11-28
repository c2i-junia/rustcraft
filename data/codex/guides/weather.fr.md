# Jour, nuit et météo

Un jour dans Rustcraft dure 20 minutes : 10 de nuit, puis 10 de jour. De temps en temps, le ciel se couvre pour de la pluie ou un orage.

## Le jour

Le cycle commence au coucher du soleil :

| Dans le cycle | Ce qui se passe |
|---|---|
| 0 min | Coucher du soleil : la nuit commence |
| 5 min | Minuit |
| 10 min | Lever du soleil : le jour commence |
| 15 min | Midi |
| 20 min | Nouveau coucher du soleil |

Un nouveau monde démarre 75 secondes après le lever du soleil : ton premier jour te laisse donc un peu moins de 9 minutes avant la nuit. Voir [[guide:first-day|ton premier jour]].

## Quand sortent les hostiles

Un hostile a besoin d’un endroit sombre pour apparaître :

- à ciel ouvert, seulement la nuit ;
- sous un toit ou sous terre, à toute heure ;
- et jamais avec une source de lumière (une [[Torch|torche]], une lanterne, un feu de camp, un four allumé, du feu, de la lave…) à moins de 8 blocs sur chaque axe.

La nuit, des [[mob:Stumpling|Souchons]] apparaissent sur l’herbe, et dans les bois la moitié d’entre eux sont des [[mob:Bramble|Ronces]]. Au-dessus de y 110, la moitié des apparitions de nuit sont des [[mob:GustOwl|Hiboux des rafales]]. Les [[mob:Drowner|Noyeurs]] n’ont pas besoin d’obscurité : ils surgissent de l’eau profonde d’au moins 3 blocs, deux fois plus souvent la nuit. À l’aube, les Souchons s’enfoncent dans le sol. Les autres hostiles restent jusqu’à ce qu’on les tue ou que tu t’éloignes (96 blocs).

Rester en vie de la première minute d’une nuit jusqu’à l’aube rapporte *Toujours là à l’aube*, mais pas si tu as dormi pendant la nuit.

## Dormir

Utilise un [[Bedroll|couchage]] pour en faire ton point de réapparition, à toute heure. Utilisé la nuit, tu t’endors aussi. Quand tous les joueurs en ligne dorment, la nuit se termine aussitôt et le soleil se lève.

{{recipe Bedroll}}

Toute couleur de laine donne un couchage de cette couleur.

## Météo

Un nouveau monde commence par un temps clair. Ensuite, le temps change tout seul :

| Temps | Dure | Vient ensuite |
|---|---|---|
| Clair | 10 à 100 min | Pluie (2 chances sur 3) ou orage (1 chance sur 3) |
| Pluie | 5 à 15 min | Clair |
| Orage | 5 à 15 min | Clair |

### La foudre

Pendant un orage, la foudre tombe toutes les 3 à 15 secondes, à 16 à 160 blocs d’un joueur. On la voit et on l’entend jusqu’à 1500 blocs. La foudre n’est qu’un spectacle et un bruit : elle ne blesse personne et ne met le feu à rien.

### Pluie et neige

{{table weather.precipitation | La neige tombe à la place de la pluie dans {biomes}. | Rien ne tombe dans {biomes}. | Partout ailleurs il pleut, et la pluie se change en neige au-dessus d’environ y 122, sur les hauts sommets. | et}}

La pluie et la neige ne sont que l’aspect de la météo. Ses effets sont les mêmes dans tous les biomes, déserts compris, partout où un bloc n’a que le ciel au-dessus de lui.

### Ce que fait la pluie

À ciel ouvert, quand il pleut ou qu’il y a de l’orage :

- elle éteint un joueur en flammes, sauf s’il est encore dans la lave ;
- elle éteint les blocs de feu ;
- elle arrose les cultures, qui poussent deux fois plus vite, comme avec de l’eau à moins de 4 blocs. Les deux ensemble ne vont pas plus vite. Voir [[guide:farming|l’agriculture]] ;
- elle tue les [[mob:Cinderling|Escarbilles]], qui ne lâchent alors rien ;
- elle remplit une [[RainCistern|citerne]]. Utilise un [[Bucket|seau]] sur une citerne pleine pour obtenir un [[WaterBucket|seau d’eau]] : de l’eau même là où il n’y en a pas autour ;
- elle éteint un [[Campfire|feu de camp]]. Rallume-le avec une torche, un [[Flint|silex]] ou un [[FlintAndSteel|briquet]].

## Commandes

Sur un serveur, elles demandent un opérateur ; dans un monde solo, tu peux toujours les utiliser. Voir [[guide:commands|les commandes]].

| Commande | Ce qu’elle fait |
|---|---|
| /time set day | Avance l’horloge jusqu’au prochain midi |
| /time set night | Avance l’horloge jusqu’au prochain minuit |
| /weather clear | Dégage le ciel |
| /weather rain | Lance la pluie |
| /weather thunder | Lance un orage |

L’horloge ne fait qu’avancer. Un temps imposé par commande dure aussi longtemps qu’un temps naturel du même genre.
