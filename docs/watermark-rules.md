# Règles de nettoyage des crédits

Document interne. Il ne part **jamais** en amont : les PR upstream ne contiennent
que `sources/en.chikari/` et `sources/en.novelbuddy/`, jamais ce dossier `docs/`.

Il décrit ce que fait `watermark.rs` et surtout **pourquoi**, avec les mesures qui
ont tranché chaque choix. Les chiffres viennent d'un corpus de 127 chapitres :
94 chapitres de quatre romans plus les chapitres 41 à 51 de quatre autres.

## Le fichier

| | |
|:--|:--|
| Code | `sources/en.chikari/src/watermark.rs` |
| Copie | `sources/en.novelbuddy/src/watermark.rs`, **identique octet pour octet** |
| Idem pour | `markdown.rs` et `mtl.rs` |
| Appelé par | `markdown.rs`, une fois **par paragraphe**, si le réglage `hide_watermark` est actif |
| Taille | 679 lignes |

Toute modification se fait dans une copie puis se recopie dans l'autre. Vérifier
après : `md5sum` des deux doit être égal.

## Le modèle

Trois définitions portent tout le reste.

**Prose** : trois blocs, plus les blancs de n'importe où.

| Bloc | Plage |
|:--|:--|
| Basic Latin | `0x0000`-`0x007F` |
| Latin-1 Supplement | `0x0080`-`0x00FF` |
| General Punctuation | `0x2000`-`0x206F` |

Les blancs Unicode s'ajoutent (`is_whitespace`), parce qu'une espace idéographique
n'est pas une marque du crédit mais un trou entre deux mots.

**Signe** : ni prose, ni alphanumérique. Un crochet CJK, un tiret cadratin exotique,
une puce.

**Étranger** : hors prose **et** alphanumérique. Une lettre ou un chiffre d'un autre
script, ce avec quoi le crédit s'écrit.

Attention : `*`, `°`, `-`, `/`, `{`, `}` sont **dans** les blocs de prose. Ils ne sont
donc jamais des signes au sens strict, mais la règle 1 les consomme quand même parce
que son test est << ni alphanumérique, ni blanc >>.

## Les règles, dans l'ordre

La chaîne actuelle comporte six étapes. Elle vise d'abord les références de site ou de
groupe qui apparaissent **au milieu du récit**. Les notes d'auteur (`Edited by`, `E/N`)
et les notes de traducteur (`T/N`) n'ont pas de statut protégé : elles peuvent rester,
être tronquées ou partir si une règle utile les touche aussi.

Une règle ne reste pas uniquement parce qu'elle supprime une note. Elle doit soit
retirer une référence de site ou de groupe, soit protéger du récit lisible, soit
réparer une structure nécessaire au nettoyage. Une suppression qui ne produit qu'un
gain sur `Edited by`, `T/N` ou `E/N` est un coût sans bénéfice et doit être retirée.

### 1. Le crédit épelé - `strip_injected`

**Détection** : une lettre, puis une alternance séparateur + lettre, **4 lettres
minimum**, en partant d'une frontière de mot. Les guillemets ne sont pas traités comme
des crochets : ils sont de la parole à garder.

**Notre résultat** (`taming-the-villainesses`, chapitre 42) :

```text
I was wondering what to °* N  v  l i g h t *° do if she ran away.
-> I was wondering what to do if she ran away.
```

**Upstream** : la ligne n'a aucun point avant le crédit, donc elle est supprimée en
entier. La phrase réelle disparaît avec le crédit.

Les marques sont retirées de chaque côté indépendamment. L'espace restant est ramené
à 1, ou au multiple de 4 inférieur pour conserver l'indentation. Le seuil est à
**4 lettres** : sur 192 chapitres, 4 et 5 donnent exactement la même sortie. Le prix
du seuil à 4 est connu : un sigle pointé de quatre lettres est pris pour un crédit.

```text
The agency is called N.A.S.A. and it is old.
-> The agency is called and it is old.
```

Le corpus n'en contient pas. On ajustera si un chapitre réel en apporte un.

Un crédit écrit à la main n'a pas toujours un séparateur unique entre deux lettres :
`°* N  v  l  i  g  h  t *°` en utilise deux. Le scan accepte donc **plusieurs**
séparateurs entre deux lettres, sinon cette forme réelle passe sans être vue. Un
crochet n'est pas un séparateur : il coupe la suite, ce qui sépare `NovlFir(.)nt` en
deux jetons au lieu de les souder.

### 2. Le marqueur de dialogue - `dialogue_line`

Le **lead** est la suite de tête faite de signes, de guillemets et d'espaces.

| Cas | Notre comportement | Upstream |
|:--|:--|:--|
| lead avec guillemet et signe, fermant trouvé après | garde l'ouvrant et la réplique | coupe selon la ligne et peut réinsérer le crédit avec le fermant |
| lead sans fermant répondant | cadratin, fermant de paire orphelin retiré | aucune règle équivalente |
| fermant de famille différente que l'ouvrant | l'ouvrant est remplacé par le guillemet qui attend ce fermant | dépend de sa réparation générique |

Le remplacement est U+2014, jamais `-` ASCII : un tiret ASCII en tête de ligne peut
créer une puce Markdown. La règle a changé 20 lignes du corpus dans les mesures de
régression précédentes.

**Le cas du fermant d'une autre famille**, parce que << conservé >> serait faux. Le
code ne garde pas l'ouvrant : il garde un guillemet, celui qui attend le fermant réel.

```text
"Bonjour," dit-il.        ouvrant " attend " ; la ligne finit par "
-> "Bonjour," dit-il.     rien ne change, le fermant est celui attendu

« Bonjour."               ouvrant « attend » ; aucun » ; la ligne finit par "
-> "Bonjour."             un fermant " attend un ouvrant " : c'est lui qui reste

« Bonjour.                ouvrant « attend » ; rien pour refermer
-> — Bonjour.             le préfixe devient un cadratin
```

La troisième ligne est le cas << orphelin >> : le fermant qui appartenait au
guillemet jeté part avec lui. C'est ce qui protège les 20 lignes du corpus qui
ouvrent et ferment sur des familles différentes : elles gardent une citation
cohérente au lieu d'un guillemet orphelin.

### 3. Les paires mortes - `without_dead_pairs`

Les guillemets sont exclus des suppressions. `PAIRS` reste la liste complète pour
l'appariement, mais les règles qui suppriment ou cherchent un groupe passent par la
version **PAIRS moins QUOTES**. Ainsi une réplique ne devient jamais un crochet de
crédit.

Pour les crochets, une pile apparie les types mélangés, puis les paires sont décidées
de l'intérieur vers l'extérieur.

| Contenu du groupe | Notre comportement | Upstream |
|:--|:--|:--|
| blanc | groupe supprimé | réparation générique |
| signes + phrases lisibles | phrases propres conservées | coupe la ligne autour du premier bruit |
| hors prose, aucune phrase entièrement propre | groupe supprimé | ligne supprimée |
| prose pure, même un mot ou deux | groupe conservé | groupe conservé |
| un seul caractère hors prose entre parenthèses | groupe conservé | selon sa réparation |

La dernière ligne est le cas de la rune : `'(\u{16C7}).'` cite un glyphe, ce n'est pas
un crédit. Le mot qui contient `(x)` avec un seul caractère hors prose est donc gardé
tel quel, avant tout nettoyage.

```text
It was the runic letter '(\u{16C7}).' in the book.   -> inchangé
It was the runic letter '(\u{16C7}).'                -> inchangé
```

La troisième ligne compte : un groupe de prose pure n'est pas jugé sur sa longueur.
`(gambas)`, `(1)`, `(a [\] c)` restent, parce qu'ils ne portent aucun caractère hors
prose. Une phrase n'est retirée que si elle contient du bruit, pas parce qu'elle est
courte.

Exemples réels, dans les deux sens :

```text
He said (to me. x done) end
-> He said (to me.) end          la phrase lisible reste, le bruit part

Edited by: faker[T/N: join the dşçöŕd (here) for spoilers, ...]
-> Edited by: faker               aucune phrase entièrement propre : le groupe part

She picked up (gambas) and left.
-> She picked up (gambas) and left.   deux mots, prose pure : rien ne change
```

La pile est importante pour les paires imbriquées. Retirer les guillemets de `PAIRS`
semble plus simple, mais casse `quote_close` et laisse des fermants orphelins.

### 4. Le découpage en phrases - `sentences`

La règle coupe sur `. ! ? ...` et les variantes doubles, avale les fermants accolés,
et exige un blanc après. `Lv. 7 > Lv. 8` donne trois phrases ; `word.word` n'en donne
qu'une.

**Mesure avec et sans cette règle sur les 97 chapitres** : 168 691 mots dans les deux
cas, sortie identique au caractère près sur les 97 chapitres. Elle reste parce que la
structure protège le cas où une ligne future porte plusieurs phrases dont une seule
contient un crédit. Le corpus actuel ne donne aucun exemple de ce bénéfice.

Upstream décide par ligne : dans le chapitre 42 de `taming-the-villainesses`, cela lui
fait supprimer toute la ligne portant `I was wondering what to ...`, alors que notre
décision par phrase conserve la phrase nettoyée.

### 5. La décision, phrase par phrase

| Ordre | Test | Notre action | Cible réelle |
|:-:|:--|:--|:--|
| 5a | domaine : point, mot avant, 2 ou 3 minuscules après | phrase supprimée | `frewebnovel.com` au milieu d'une phrase |
| 5b | prose pure (test d'entrée) | phrase gardée telle quelle | récit normal et notes en prose |
| 5c | crédit : jeton cassé, crochet touchant, mots étrangers | nettoyage, ou `None` | `fr(e)webnovel`, `* Nvlght * (Only on Nvlgt)` |
| 5d | rien de lisible | phrase supprimée | `.`, `*` et fragments vides |

**5a - domaine.** Le domaine est une cible prioritaire : une phrase qui contient un
site est supprimée entière. Upstream peut aussi supprimer la ligne, mais sa découpe
peut prendre du récit autour du domaine.

**5b - prose (test d'entrée, pas une décision).** Le test intervient avant le
nettoyage, pour que 5c ne touche pas une phrase qui n'a rien à nettoyer. Il n'est pas
une règle de décision et il occupe donc sa place actuelle : après le domaine, avant
le crédit. Le déplacer plus bas ferait passer une phrase propre dans le nettoyage mot
par mot, qui retire les signes isolés et transforme une suite de signes entre deux
lettres en espace. La sortie resterait identique sur le corpus, mais la garde
disparaîtrait pour une phrase de prose contenant `*` ou `-`.

Le retirer est mesuré sans effet sur les 97 chapitres. Il est gardé comme garde
d'identité, et non comme règle de nettoyage.

Une phrase de récit pure reste toujours. Une note d'auteur ou de traducteur en prose
peut aussi rester, mais ce n'est pas une garantie à préserver : une règle qui retire
une référence de site peut retirer la note voisine avec elle. Cette branche n'est
pas une règle de suppression de notes.

**5c - jeton cassé (première étape de 5c).** Un jeton qui porte une lettre, une
parenthèse, un caractère unique, la fermeture, puis encore une lettre est un domaine
découpé pour échapper aux filtres. La phrase part entière.

```text
Read latest chapters at f(r)eewebnovl Only     -> supprimé
The source is free(w)bnov()l                   -> supprimé
This is fr(e)ewebnovel                         -> supprimé
runic letter '(\u{16C7}).'                     -> conservé
He paused (briefly) and left.                  -> conservé
```

La condition exige une lettre **avant** et **après** la parenthèse, dans le même
jeton : un caractère seul entre deux espaces, ou en fin de jeton, ne déclenche rien.
C'est ce qui sauve la rune.

La règle est évaluée **dans** le nettoyage de crédit, pas avant le test de prose. Les
deux placements donnent la même sortie sur le corpus, parce qu'un tel jeton contient
une parenthèse et n'entre donc jamais dans la branche de prose pure. Le placement
interne garde la ligne de décision à trois tests.

**5c - crédit et parenthèse adjacente.** D'abord, `bracket_close` lit `PAIRS` mais
refuse les ouvertures de `QUOTES`. Le groupe qui touche un jeton portant une lettre
étrangère est retiré avant le passage mot par mot. Deux branches seulement, pas trois :
le crochet **après** le jeton, et le crochet **avant** le jeton. Entre le jeton et le
crochet, le code saute indifféremment blancs et signes, donc << collé >> n'est pas un
cas distinct : c'est le même saut.

```text
* Nvlght * (Exclusive on Nvlght)    espaces et signes autour du crochet
* Nvlght *(Exclusive on Nvlght)     collé : même branche
* Nvlght *\t(Only on Nvlgt)          blanc quelconque : même branche
```

Sur le corpus de 216 chapitres, la branche << avant >> et les formes collées
n'ajoutent aucun retrait mesurable : la sortie est identique au caractère près. Elles
restent comme protection.

Ensuite les signes sont retirés, les signes entre deux lettres deviennent une espace,
et le mot qui garde une lettre étrangère part.

Le corpus donne cinq retraits, quatre formes :

| Entrée réelle | Notre sortie | Sortie upstream |
|:--|:--|:--|
| `...tweaked. * Nvlght * (Exclusive on Nvlght) For example...` | `...tweaked. (For example...` | le texte revient avec la réparation du guillemet |
| `...entry, * Nvlgt * (Only on Nvlgt) you'll...` | parenthèse supprimée | crédit conservé |
| `...ticking time * NvIight * (Original source) bombs...` | parenthèse supprimée | crédit conservé |
| `...many * Nvlght * (Read the full story) people...` | parenthèse supprimée | crédit conservé |

Les deux occurrences de `(Read the full story)` sont dans les chapitres 46 et 49.
La branche avant et les formes collées sont désormais réellement couvertes : le scan
s'arrête sur un crochet, donc `Novlght(Exclusive on ...)` est vu comme un jeton suivi
d'un groupe, et non comme un seul bloc.

```text
He fixed it. Nvlght (Exclusive on Nvlght) Then he left.   -> groupe retiré
He fixed it. Nvlght(Exclusive on Nvlght) Then he left.    -> groupe retiré
He fixed it. (Only on Nvlght) Nvlght Then he left.        -> groupe retiré
```

La branche avant est restreinte aux fermants non ASCII sans chiffre ASCII dans le
jeton, sinon un numéro de chapitre `220 (` se ferait prendre pour un crochet. Une note `T/N`, `E/N` ou `Edited by` peut toutefois être
touchée si elle porte le même motif étranger adjacent : sa conservation n'est pas une
contrainte du modèle, mais cette possibilité ne justifie pas à elle seule la règle.

La récupération de prose étrangère fait partie de **5c**. Elle ne s'applique que si
la phrase ne contient **qu'un seul** mot porteur d'une lettre étrangère, sans point
interne de domaine : `Fųck, this hurt!` devient `Fck, this hurt!`. Plusieurs mots
étrangers dans la même phrase, c'est le crédit qui s'épelle à travers elle, et la
phrase part.

```text
Fųck, this hurt!                                  -> Fck, this hurt!   un mot, récupéré
fluffy-Th ... orign ... rsts n ... NovlFir(.)nt   -> phrase nettoyée   cinq mots, crédit
```

Le seuil `<= 2` a été mesuré : il réintroduit `NvIight` dans `taming` 44, qui est un
crédit. Une seule lettre étrangère est donc le seuil retenu. Il n'y a plus de 5d séparée.

### 6. Le recollage - `strip`

Les phrases sont recollées par une espace, les lignes par `\n`, et la fin est rognée.
Le caller de `markdown.rs` recolle ensuite les blocs par une ligne vide.

### Résultat global contre upstream

Sur 97 chapitres et 168 905 mots, après conversion HTML identique et application bloc par
bloc :

| Mesure | Notre version | Upstream |
|:--|--:|--:|
| Mots retirés | **214** | **1 514** |
| Chapitres modifiés | **58** | **58** |
| Caractères hors prose restants | **0** | **20** |
| Lettres étrangères restantes | **0** | **14** |
| Chapitres encore sales | **0** | **3** |

Upstream retire sept fois plus de mots, principalement parce qu'il supprime des lignes
entières. Nous retirons moins, mais nous retirons la cible prioritaire : les références
de site ou de groupe au milieu du texte. Les notes d'auteur et de traducteur ne sont
pas protégées dans l'une ou l'autre sortie.

## Les listes

| Liste | Contenu | Sert à |
|:--|:--|:--|
| `PROSE_BLOCKS` | 3 blocs | décider ce qui est du texte |
| `PAIRS` | 11 paires | apparier et fermer une citation |
| `QUOTES` | 10 caractères | reconnaître de la parole |
| `SENTENCE_ENDS` | 9 caractères | couper les phrases |
| `SPELLED_MIN_LETTERS` | 4 | seuil de l'épellation |

`PAIRS` sert à **supprimer**, `QUOTES` seulement à **lire**. C'est pourquoi les
guillemets ASCII sont dans `QUOTES` et pas dans `PAIRS` : ils s'écrivent seuls
(pouces, apostrophe) et y mettre exposerait à supprimer du texte.

## Mesures de référence

Second corpus, 216 chapitres : 24 par roman et par source, huit premiers, huit au
milieu, huit derniers. Cinq romans : `shadow-slave`, `chaotic-craftsman-worships-the-cube`,
`city-of-witches`, `taming-the-villainesses` et `delve`. Delve n'existe que sur
Novelbuddy.

| Mesure | Valeur |
|:--|--:|
| Chapitres | 216 |
| Mots bruts | 437 154 |
| Mots en sortie | 436 753 |
| Mots retirés | 401 |
| Caractères hors prose | 0 |
| Chapitres sales | 0 |
| Écart avec la version publiée `6b091a6` | 11 chapitres, tous du crédit |



Contre la v32 publiée, sur 127 chapitres :

| Mesure | Valeur |
|:--|--:|
| Chapitres changés | **20** |
| Mots perdus | **23** |
| Mots gagnés | **28** |
| Caractères hors prose en sortie | **0** |
| Lettres étrangères en sortie | **0** |

Les 23 mots perdus sont tous du crédit, des fragments de domaine mutilés, ou la forme
recollée `FarsightCalm` qui devient deux mots.

Les 28 gagnés sont de la prose réelle (`Farsight`, `Calm`, `erased`, `doves`,
`daring`, `encourage`, `absolute`, `oppose`) plus **trois débris** de crédit.

Chapitres 22 à 28 de `shadow-slave`, `city-of-witches` et `chaotic-craftsman` :
**21 identiques**. Les changements sont localisés sur les chapitres qui portent un
crédit.

## Comparaison avec l'implémentation upstream

L'upstream (`Aidoku-Community/sources`) fait 200 lignes et 10 fonctions. La nôtre en
fait 599 et 31 fonctions. Ce n'est pas la même idée : upstream coupe des lignes, nous décidons
des phrases.

### Méthode

| Point | Choix |
|:--|:--|
| Corpus | 97 chapitres, 168 905 mots : 65 `en.chikari` et 32 `en.novelbuddy` |
| Entrée | le texte du chapitre, les 59 chapitres arrivés en HTML étant convertis d'abord, comme le fait `markdown.rs` |
| Application | bloc par bloc, exactement comme `markdown.rs` appelle `watermark::strip` |
| Référence << sans watermark >> | les mêmes blocs recousus, réglage éteint |
| Égalité de traitement | les deux implémentations reçoivent le même texte, au caractère près |

### Résultat global

| Mesure | Brut | Notre | Upstream |
|:--|------:|------:|------:|
| Mots | 168 905 | 168 691 | 167 391 |
| Mots retirés | - | **214** | **1 514** |
| Lignes | 18 187 | 18 173 | 17 895 |
| Chapitres modifiés | - | 58 | 58 |
| Sorties différentes | - | 49 chapitres | 49 chapitres |
| Caractères hors prose restants | 358 | **0** | 20 |
| Lettres étrangères restantes | 106 | **0** | 14 |
| Chapitres encore sales | 53 | **0** | 3 |

Upstream retire donc sept fois plus de mots, et laisse pourtant du crédit derrière
lui. Les deux résultats ne sont pas du même ordre : il jette du texte pour retirer
un crédit, nous retirons le crédit.

### L'algorithme upstream, en trois étapes

| Étape | Ce qu'elle fait |
|:--|:--|
| 1. Blocs | découpe sur les lignes vides |
| 2. Ligne | si la ligne n'a aucun caractère hors prose, elle est gardée telle quelle ; sinon on coupe au **dernier** point avant ce caractère, et on jette la ligne s'il n'y en a aucun |
| 3. Réparation | pour un guillemet laissé ouvert, on cherche son fermant dans le reste et on **réinsère tout jusqu'à lui** ; pour un crochet, on ajoute simplement le fermant |

**Bénéfice** : dix fonctions, aucun seuil, aucun réglage, et sur du texte propre elle
ne touche rien.

**Compromis mesurés** :

| Défaut | Mesure |
|:--|:--|
| Perte de prose | **1 174 occurrences** de mots de prose qu'elle retire et que nous gardons (`the` 60 fois, `to` 24, `have` 24, `is` 23) |
| Crédit laissé | 3 chapitres gardent des caractères hors prose, dont un crédit entier |
| Réparation contre-productive | l'étape 3 réinsère le texte que l'étape 2 venait de couper, dès que la coupe tombe dans une réplique |

### Nos six règles, bénéfice et compromis

| Règle | Bénéfice mesuré | Compromis mesuré |
|:--|:--|:--|
| 1. Crédit épelé | retire le crédit au milieu d'une phrase **sans perdre la phrase** | les signes accolés partent avec |
| 2. Marqueur de dialogue | garde la réplique et son guillemet | un tiret de tête devient un cadratin |
| 3. Paires mortes | supprime les blocs de crédit entre crochets, garde les répliques citées | une paire en prose pure reste |
| 4. Phrases | décide par phrase, pas par ligne | `Lv. 7` compte pour une phrase ; avec/sans sur le corpus : 0 différence |
| 5. Décision | 214 mots retirés là où upstream en retire 1 514 ; les références de site du milieu partent | la note de traducteur de `city-of-witches` 25 sort tronquée |
| 6. Recollage | un paragraphe reste un paragraphe | aucune |

### Où chacun gagne

| Cas | Qui gagne | Pourquoi |
|:--|:--|:--|
| Crédit au milieu d'une phrase, aucun point avant lui | **nous** | upstream jette la ligne entière |
| Crédit dans une réplique citée | **nous** | la réparation d'upstream réinsère le crédit |
| Note d'auteur en prose pure | les deux peuvent la garder ou la supprimer | elle n'est pas protégée |
| Titre de chapitre seul dans son paragraphe | **nous** | upstream supprime les 22 titres du corpus, nous les gardons |
| Note de traducteur | comportement variable | une note sur 15 sort tronquée chez nous, et cette troncature est acceptable |

### Mesure : retirer la parenthèse qui suit un crédit

Idée mesurée : après un crédit, la parenthèse accolée ne dit rien de plus, et c'est
elle qui laissait `(Exclusive on)` en sortie avant le renforcement de 5c.

| Règle testée | Portée | Résultat |
|:--|:--|:--|
| parenthèse après un crédit **épelé** (règle 1) | 97 chapitres | **0 occurrence** : ce crédit ne porte jamais de parenthèse ici |
| parenthèse après un jeton **portant une lettre étrangère** (règle 5c) | 97 chapitres | **5 retraits**, 4 formes, **-14 mots**, 0 faux positif |

Ce que la seconde retire, exhaustivement :

| Forme | Occurrences | Chapitre |
|:--|--:|:--|
| `(Exclusive on Nvlght)` | 1 | taming 43 |
| `(Only on Nvlgt)` | 1 | taming 51 |
| `(Original source)` | 1 | taming 44 |
| `(Read the full story)` | 2 | taming 46, 49 |

Le faux positif à craindre est un texte du récit qui cite un caractère d'un autre
script entre parenthèses. Le corpus en contient un : `the runic letter '(...)'`, où la
parenthèse **contient** le caractère au lieu de le **suivre**. La règle ne le touche
pas, parce qu'elle exige que le jeton étranger précède la parenthèse.

Le crochet **avant** le jeton et le crochet **collé**, sans espace, sont dans la règle
retenue. Mesurés, ils n'ajoutent aucun cas sur ce corpus : les cinq retraits sont les
mêmes et la sortie est identique au caractère près sur les 97 chapitres. Ils sont là
comme sûreté, pas comme gain.

La règle lit `PAIRS`, mais **jamais un guillemet** : `bracket_close` refuse toute
ouverture qui figure dans `QUOTES`, exactement comme la règle des paires mortes. Sans
cette garde, un jeton étranger suivi d'une réplique citée aurait emporté la réplique
entière. Le corpus n'a pas ce cas, la garde est donc gratuite ici et protège là où il
apparaîtrait.

### Mesure : la règle de forme, sans vocabulaire

Objectif : retirer un crédit fait de prose pure, seul dans son paragraphe, sans liste
de vocabulaire. Forme essayée : un bloc de quatre mots au plus, sans chiffre, sans
ponctuation de fin de phrase, contenant un deux-points.

| Variante | Blocs retirés | Crédits | Fiches de statut | Répliques | Titres de section |
|:--|--:|--:|--:|--:|--:|
| forme large | 49 | 21 | 10 | 4 | 14 |
| forme étroite, sans deux-points final | 35 | 21 | 10 | 4 | 0 |

Les positifs, tous réels :

| Bloc | Où |
|:--|:--|
| `Edited by: faker` | taming 41, 42, 44, 45, 47, 49, 50, 51, et quatre chapitres novelbuddy |
| `E/N:[Insert Giga Chad]` | city-of-witches 22, et un chapitre novelbuddy |

Les négatifs, tous réels :

| Bloc | Ce que c'est | Où |
|:--|:--|:--|
| `"Name: Nephis"` | une réplique | shadow-slave 24, et un chapitre novelbuddy |
| `"True Name: Changing Star"` | une réplique | shadow-slave 24, et un chapitre novelbuddy |
| `Talents: Farsight Calm Thinking` | une fiche de statut, donc du récit | taming 47, 48 |
| `Meanwhile, the instructor continued:` | une ligne de narration | shadow-slave 23 |

`"Name: Nephis"` et `Edited by: faker` ont exactement la même forme : quatre mots au
plus, un deux-points, aucun chiffre, aucun point final. Aucune règle de forme ne les
sépare sans lire les mots, donc sans vocabulaire. Les deux variantes jettent 14 blocs
de récit, dix fiches et quatre répliques, pour 21 crédits, et la variante étroite jette
en plus les titres de section. Verdict : écartée.

### Exemples réels

**A. Crédit épelé au milieu d'une phrase** (`taming-the-villainesses` ch. 42)

| | |
|:--|:--|
| brut | `I was wondering what to °* N  v  l i g h t *° do if she ran away.` |
| notre | `I was wondering what to do if she ran away.` |
| upstream | **ligne absente du chapitre** |

La ligne n'a aucun point avant le crédit : upstream la jette en entier. Nous retirons
le crédit et gardons la phrase.

**B. Crédit à lettres ressemblantes, dans une réplique** (`taming-the-villainesses` ch. 43)

| | |
|:--|:--|
| brut | `"Of course, there are some things that need to be fixed or tweaked. * Nvlght * (Exclusive on Nvlght) For example, the name 'slave' is a bit out of date."` |
| notre | `"Of course, there are some things that need to be fixed or tweaked. For example, the name 'slave' is a bit out of date."` |
| upstream | **identique au brut** |

Upstream coupe bien après `tweaked.`, puis son étape 3 voit le guillemet ouvert et
réinsère tout jusqu'au guillemet fermant : le crédit revient. Nous retirons le crédit
et sa parenthèse de site, puis gardons la phrase réelle qui suit.

**C. Titre de chapitre, seul dans son paragraphe** (`city-of-witches` ch. 22)

| | |
|:--|:--|
| brut | `U+273A U+273A Amelia Merigold (1) U+273B` |
| notre | `Amelia Merigold (1)` |
| upstream | **paragraphe supprimé** |

Le label du chapitre est `Chapter 22: Amelia Merigold (1)` : ce paragraphe est le
titre, pas un crédit. Upstream le supprime parce que les marques décoratives sont
hors prose ; nous le gardons. Les 22 paragraphes de cette forme dans le corpus sont
tous des titres, donc l'avantage supposé d'upstream sur le crédit en prose pure
n'existe pas ici.

**D. Note de traducteur** (`city-of-witches` ch. 25)

| | |
|:--|:--|
| brut | `[T/N- ""/ Gambas, is a Spanish dish with shrimp and garlic as the main ingredients. The name means "shrimp (gambas) and garlic (ajillo)" in Spanish...` |
| notre | `[The name means "shrimp (gambas) and garlic (ajillo)" in Spanish...` |
| upstream | garde une autre note, entière |

Notre découpage par phrases retire la première phrase de la note et garde la suite :
la note commence au milieu. C'est le défaut le plus visible de notre approche sur ce
corpus.

### Ce que la comparaison ne dit pas

Elle ne mesure pas la qualité littéraire : un mot retiré n'est pas toujours une perte
(`the` dans un crédit ne vaut pas `the` dans une phrase). Elle ne couvre pas non plus
les chapitres sans crédit, où les deux implémentations ne changent rien.

## Limites connues

| Cas | Comportement | Statut |
|:--|:--|:--|
| Crédit entièrement en prose (`Nvlght`, `frewebnovel.com`) | laissé | accepté, aucune règle de vocabulaire |
| Titre de chapitre seul dans son paragraphe (`Amelia Merigold (1)`) | conservé | correct : c'est le titre, upstream le supprime |
| Notes d'auteur et de traducteur (`Edited by: faker`, `T/N`, `E/N:`) | conservées, tronquées ou supprimées selon les autres règles | accepté : elles ne sont pas protégées |
| Référence de site ou de groupe au milieu du texte (`(Exclusive on)`, `(Only on)`, `(Original source)`, `(Read the full story)`) | **retirée** par la règle 5c | résolu, voir la mesure plus bas |
| Note de traducteur commençant par une phrase retirée | sort tronquée (`city-of-witches` 25) | accepté, la note n'est pas du récit |
| Phrase portant un domaine | supprimée entière | accepté |
| Crochets mélangés mal formés `([dead) useful]` | laissés intacts | accepté |

## Décisions écartées, avec leur mesure

| Idée | Mesure | Verdict |
|:--|:--|:--|
| Seuil d'épellation à 4 | 0 chapitre | écarté, risque de sigle |
| Consommer jusqu'à l'alphanumérique | 3 chapitres, -3 signes | écarté, mange la ponctuation |
| Guillemets hors de `PAIRS` | 13 chapitres, +50 / -5 | écarté, casse l'appariement |
| Retirer les guillemets mal appariés | 20 lignes concernées | écarté, ponctuation légitime |
| Passe intérieure explicite pour les crochets | 0 chapitre | appliqué quand même, plus juste |
| Logique des parenthèses pour la règle 5 | 29 chapitres, **-202 mots** | écarté, jette les phrases et les fiches |
| Parentèse courte jetée (option 2) | 46 chapitres, **-146 mots** | écarté, jette `(gambas)`, `(1)`, les incises |
| Crédits nettoyés avant les guillemets | 0 chapitre | neutre, ordre actuel conservé |
| Guillemets corrigés en dernier | 29 chapitres, **-86 mots** | écarté |
| Règle de forme sur un bloc court avec deux-points | 35 à 49 blocs, dont 14 de récit | écarté, ne sépare pas `"Name: Nephis"` de `Edited by: faker` |
| Crochet avant et crochet collé, en plus de l'après | 0 cas de plus, sortie identique | **appliqué** comme sûreté, pas comme gain |

## Points ouverts

- Retirer le guillemet orphelin **en fin** de ligne : la variante est écrite, elle ne
  se déclenche sur aucun chapitre. Le `+2` du chapitre 41 de `city-of-witches` n'est
  pas un défaut mais un fermant légitime conservé.
- Les trois débris de crédit : seul un vocabulaire les enlèverait.
- Version publiée : 32 / 37. Toute modification de comportement exige un bump pour
  atteindre un appareil.

## Vérifications avant de livrer

```sh
# identité des deux copies
md5sum sources/en.chikari/src/watermark.rs sources/en.novelbuddy/src/watermark.rs

# par crate
cd sources/en.chikari && cargo +1.99.0 fmt --check
cd sources/en.chikari && cargo +1.99.0 clippy --target wasm32-unknown-unknown -- -D warnings
cd sources/en.chikari && cargo +1.99.0 test
```
