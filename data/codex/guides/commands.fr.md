# Commandes du chat

Les commandes se tapent dans le chat : appuie sur **T**, ou sur **/** pour ouvrir le chat avec la barre oblique déjà tapée. Pendant que tu écris, le chat propose des noms de commandes, des arguments et des noms d’objets : prends une suggestion avec **Tab**, passe de l’une à l’autre avec **Haut** et **Bas**. Une ligne qu’il sait déjà fausse affiche ce que le serveur répondrait.

## Qui peut les utiliser

- **En solo**, tu peux lancer toutes les commandes.
- **Sur un serveur**, seule */list* est ouverte à tous. Toutes les autres commandes, y compris celles ajoutées par des mods, sont réservées aux **opérateurs** ; les autres reçoivent « You must be an operator to use /… ». Voir [[guide:multiplayer|le multijoueur]] pour le devenir.
- La réponse à une commande n’est envoyée qu’à celui qui l’a tapée.

## Toutes les commandes

- */time set <day|night>* : saute au prochain midi ou à la prochaine minuit.
- */give <objet> <quantité>* : te donne des objets.
- */gamemode <creative|survival>* : change ton propre mode de jeu.
- */weather <clear|rain|thunder>* : fixe la météo.
- */difficulty <peaceful|easy|normal|hard>* : fixe la difficulté du monde.
- */locate <lieu>* : trouve le biome, village, donjon, naufrage ou biome de grotte le plus proche.
- */list* : nomme les joueurs en ligne. Ouverte à tous.
- */op <joueur>*, */deop <joueur>* : fait d’un joueur un opérateur, ou lui retire ce rang.
- */kick <joueur> [motif]* : déconnecte un joueur.
- */ban <joueur> [motif]*, */pardon <joueur>* : bannit un joueur, ou lève le bannissement.
- */whitelist <on|off|list>*, */whitelist <add|remove> <joueur>* : gère la liste blanche.

Les mots entre *<chevrons>* sont obligatoires ; *[motif]* peut être omis. *a|b* signifie l’un des deux mots. Les mots-clés comme *day*, *creative* ou *add* s’écrivent tels quels, en anglais.

## /give

- */give oak_log 64*
- */give iron_ingot 10*
- */give diamond_sword 1*

La quantité est obligatoire, et doit être un nombre entier supérieur à 0. Les objets vont à toi, le joueur qui a tapé la commande. Ce qui ne tient pas dans ton inventaire tombe à tes pieds, jusqu’à 36 piles de plus ; au-delà, c’est perdu, et la réponse dit combien.

### Noms d’objets

Le nom d’un objet est son nom anglais en minuscules, avec des tirets bas à la place des espaces : [[OakLog|la bûche de chêne]] est *oak_log*, [[IronIngot|le lingot de fer]] est *iron_ingot*. La casse n’a pas d’importance, et *OakLog* fonctionne aussi. Le chat suggère les noms pendant que tu les tapes.

Quelques objets ont un nom de commande différent de celui que tu vois :

{{table commands.renamed | Objet | Nom de commande}}

Tous les objets figurent sur la [[page:items|page des objets]].

## /time

*/time set day* saute au prochain midi, */time set night* à la prochaine minuit. L’horloge ne recule jamais, et sur un serveur l’heure change pour tout le monde. Voir [[guide:weather|le jour, la nuit et la météo]].

## /weather

*/weather clear*, */weather rain* ou */weather thunder* lance cette météo pour la durée qu’elle aurait eue d’elle-même, puis le cycle reprend comme d’habitude. Elle s’applique à tout le monde.

## /gamemode

*/gamemode creative* ou */gamemode survival* change seulement ton propre mode de jeu ; il n’y a pas de mode aventure. La casse n’a pas d’importance. Quitter le créatif en volant te fait atterrir. Voir [[guide:game-modes|les modes de jeu]].

## /difficulty

*/difficulty peaceful*, *easy*, *normal* ou *hard* fixe la difficulté du monde. Paisible retire toutes les créatures hostiles dès le tick suivant. L’effet de la difficulté sur les dégâts des hostiles est expliqué sur la [[guide:combat|page du combat]].

## /locate

*/locate* trouve le lieu le plus proche d’un genre donné et répond avec ses coordonnées et sa distance, par exemple « The nearest village is at 412 -880, 976 blocks away ». Une recherche peut prendre un moment ; la réponse arrive quand elle est terminée.

| Commande | Trouve | Les coordonnées de |
|---|---|---|
| */locate biome <biome>* | Un biome, à moins de 4096 blocs | Une colonne de celui-ci (X et Z) |
| */locate village* | Un village, à moins de 4096 blocs | Son centre (X et Z) |
| */locate ruin* | Une ruine, à moins de 2048 blocs | Sa porte (X, Y et Z) |
| */locate crypt* | Une crypte, à moins de 2048 blocs | Sa porte (X, Y et Z) |
| */locate sanctuary* | Un sanctuaire, à moins de 2048 blocs | Sa porte (X, Y et Z) |
| */locate shipwreck* | Une épave, à moins de 2048 blocs | Son coffre (X, Y et Z) |
| */locate lush_caves* | Des grottes luxuriantes, à moins de 2048 blocs | Un endroit à l’intérieur (X, Y et Z) |
| */locate crystal_caves* | Des grottes de cristal, à moins de 2048 blocs | Un endroit à l’intérieur (X, Y et Z) |
| */locate basalt_galleries* | Des galeries de basalte, à moins de 2048 blocs | Un endroit à l’intérieur (X, Y et Z) |

- Les grottes luxuriantes, les grottes de cristal et les galeries de basalte n’existent que dans les mondes profonds.
- Un monde créé sans villages ni donjons répond qu’il n’en a pas.
- Voir [[guide:structures|les structures]] pour ce que contiennent les villages et les donjons.

### Clés de biomes

*/locate biome* prend la clé d’un biome, comme */locate biome jungle*. Un biome ajouté par un mod s’écrit avec son espace de noms, comme *mymod:name* ; ceux du jeu de base peuvent aussi s’écrire *rustcraft:jungle*.

{{table commands.biomes | Biome | Clé}}

## /list

*/list* répond avec les joueurs en ligne, par ordre alphabétique : « 2 players online: Alex, Steve ». Tout le monde peut la lancer.

## Commandes d’opérateur

Elles gèrent qui peut jouer sur un serveur dédié.

- */op Alex*, */deop Alex* : font d’Alex un opérateur, ou lui retirent ce rang.
- */kick Alex too loud* : déconnecte Alex, qui voit « You were kicked from the server: too loud ».
- */ban Alex griefing* : déconnecte Alex et le refuse dorénavant, avec le motif. */pardon Alex* lève le bannissement.
- */whitelist on* et */whitelist off* : quand elle est activée, seuls les joueurs de la liste blanche et les opérateurs peuvent rejoindre. */whitelist add Alex*, */whitelist remove Alex* et */whitelist list* la gèrent.

Les noms sont comparés sans tenir compte de la casse. Sauf pour */kick*, le joueur n’a pas besoin d’être en ligne. Un nom qui contient des espaces se met entre guillemets : */ban "Bad Guy"*. Sur un monde hébergé, les membres se gèrent plutôt depuis l’onglet Mondes hébergés, et seule */kick* fonctionne.
