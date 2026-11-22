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
| Taille | 519 lignes |

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

### 1. Le crédit épelé - `strip_injected`

**Détection** : une lettre, puis une alternance séparateur + lettre, **5 lettres
minimum**, en partant d'une frontière de mot.

**Retrait des marques** : de chaque côté, **indépendamment**. On saute les espaces
collés à l'injection, puis on consomme la suite de caractères ni alphanumériques ni
blancs, et on s'arrête.

**Recollage** : l'espace laissé est ramené à 1, ou au multiple de 4 inférieur, pour
que l'indentation garde son cran.

| Entrée réelle | Sortie |
|:--|:--|
| `"I /N_o_v_e_l_i_g_h_t/ heard there is a village nearby."` | `"I heard there is a village nearby."` |
| `Someone then smacked me {N*o*v*e*l*i*g*h*t} on the back.` | `Someone then smacked me on the back.` |
| `I was wondering what to °* N v l i g h t *° do if she ran away.` | `I was wondering what to do if she ran away.` |

**Écarté** : passer le seuil à 4 lettres. Mesuré 0 chapitre de différence, et cela
retirerait un vrai sigle écrit avec séparateurs (`N.A.S.A.`).

**Écarté** : consommer jusqu'au premier alphanumérique, blancs compris. Mesuré 3
chapitres et **mange de la ponctuation** : `little,` > `little`, `that.` > `that`,
`workers'` > `workers`.

### 2. Le marqueur de dialogue - `dialogue_line`

Le **lead** est la suite de tête faite de signes, de guillemets et d'espaces.

| Cas | Action |
|:--|:--|
| le lead n'a pas à la fois un guillemet et un signe | `dialogue_marks` |
| le lead a les deux, et un fermant apparaît après le lead | on garde l'ouvrant |
| le lead a les deux, et aucun fermant n'apparaît | cadratin, et le fermant de paire orphelin part |

`dialogue_marks` : si la ligne finit par un signe, `wrapped_text` garde l'intérieur ;
sinon la suite de signes de tête devient un cadratin.

Le remplacement est **U+2014**, jamais `-` ASCII : en markdown, un `-` en début de
ligne crée une puce. Un tiret déjà présent n'est jamais touché, parce que `-`, `-` et
`-` sont dans les blocs de prose.

| Entrée réelle | Sortie |
|:--|:--|
| `\u{0F3A}\u{0F3A} Amelia Merigold (1) \u{0F3B}` | `Amelia Merigold (1)` |
| `"-Hello," she said.` (ferme au milieu) | `"Hello," she said.` |

**Décidé** : ne pas retirer un fermant mal apparié. 20 lignes du corpus ouvrent et
ferment sur des guillemets de familles différentes, et le fermant est chaque fois
légitime.

### 3. Les paires mortes - `without_dead_pairs`

**Les guillemets sont exclus** : une réplique citée est de la parole à garder, pas un
crochet à couper. `PAIRS` les garde pour `quote_close` et `matching_opener`, qui en
ont besoin pour apparier.

Pour les crochets, une pile apparie chaque ouvrante à sa fermante, types mélangés
compris, puis les paires sont décidées **de l'intérieur vers l'extérieur**.

| Contenu | Action |
|:--|:--|
| blanc | la paire part |
| hors prose, avec des phrases lisibles | on garde les phrases dont aucun mot n'est bruité |
| hors prose, aucune phrase lisible | la paire et son contenu partent |
| tout en prose | la paire reste |

| Entrée réelle | Sortie |
|:--|:--|
| `Edited by: faker[T/N: join the dşçöŕd (here) for spoilers, ...]` | `Edited by: faker` |
| `He said (a [\] c) then left.` | `He said (a c) then left.` |

L'intérieur d'abord change la sortie : juger l'extérieure en premier l'emportait avec
le crochet intérieur. Mesuré 0 chapitre de différence, mais la forme est meilleure.

**Sans cette exclusion des guillemets** : une réplique entière disparaissait dès
qu'un signe s'y trouvait. 18 lignes perdues sur 8 656, dont deux de vrai dialogue.

**Écarté** : retirer les guillemets de `PAIRS`. Mesuré 13 chapitres, +50 / -5 mots,
mais `quote_close` ne sait alors plus apparier, et un fermant reste orphelin.

### 4. Le découpage en phrases - `sentences`
Coupe sur `. ! ? ...` et les variantes doubles, avale les fermants accolés, **exige un
blanc** après. `Lv. 7 > Lv. 8` donne trois phrases ; `word.word` n'en donne qu'une.

### 5. La décision, phrase par phrase
| Ordre | Test | Action | Exemple réel |
|:-:|:--|:--|:--|
| 5a | domaine (un point, un caractère de mot avant, 2 ou 3 minuscules après) | supprime la phrase | `The source of this cntent is frewebnovel.com.` |
| 5b | prose pure | garde | `This is to make it cohesive.` |
| 5c | un crédit d'un seul tenant | voir ci-dessous | - |
| 5d | repli | garde les caractères de prose, s'il reste un mot | `{1} Explanation:  is Jeoksangari, ...` |
| 5e | rien de lisible | supprime | `.`, `*` |

**5c** : d'abord, retirer le crochet qui touche un jeton porteur d'une lettre étrangère,
avant comme après, collé ou non. Puis, pour chaque mot, retirer les signes ; une suite
de signes entre deux lettres devient **une espace** ; jeter le mot s'il porte encore une
lettre étrangère ; garder le reste. Le crochet part parce que ses mots sont de la prose :
sans cette étape, le passage mot par mot les garderait et laisserait `(Exclusive on)`.
| Entrée réelle | Sortie |
|:--|:--|
| `... traces of \ \ her past through the Thieves' Guild?` | phrase entière |
| `talent\u{300A}Farsight\u{300B}being activated` | `talent Farsight being activated` |
| `Talents: \u{300A}Farsight\u{300B}\u{300A}Calm\u{300B} Thinking` | `Talents: Farsight Calm Thinking` |

Les crochets CJK U+300A / U+300B du corpus servent de **séparateurs de mots** : les
transformer en espace est le comportement correct.

**5d** reste utile : quand **tous** les mots portent une lettre étrangère, 5c ne rend
rien et 5d sauve les caractères de prose (`Fųck` seul donne `Fck`). Sur les 97 chapitres
du corpus, retirer 5d ne change **aucun** chapitre ; la règle ne se déclenche donc pas
ici, mais elle est la seule à récupérer ce cas.

### 6. Le recollage
Phrases par une espace, lignes par `\n`, fin rognée.
## Les listes

| Liste | Contenu | Sert à |
|:--|:--|:--|
| `PROSE_BLOCKS` | 3 blocs | décider ce qui est du texte |
| `PAIRS` | 11 paires | apparier et fermer une citation |
| `QUOTES` | 10 caractères | reconnaître de la parole |
| `SENTENCE_ENDS` | 9 caractères | couper les phrases |
| `SPELLED_MIN_LETTERS` | 5 | seuil de l'épellation |

`PAIRS` sert à **supprimer**, `QUOTES` seulement à **lire**. C'est pourquoi les
guillemets ASCII sont dans `QUOTES` et pas dans `PAIRS` : ils s'écrivent seuls
(pouces, apostrophe) et y mettre exposerait à supprimer du texte.

## Mesures de référence

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
fait 520 et 27. Ce n'est pas la même idée : upstream coupe des lignes, nous décidons
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
| Mots | 168 905 | 168 703 | 167 391 |
| Mots retirés | - | **202** | **1 514** |
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
| 5. Décision | 216 mots retirés là où upstream en retire 1 514 ; les références de site du milieu partent | la note de traducteur de `city-of-witches` 25 sort tronquée |
| 6. Recollage | un paragraphe reste un paragraphe | aucune |

### Où chacun gagne

| Cas | Qui gagne | Pourquoi |
|:--|:--|:--|
| Crédit au milieu d'une phrase, aucun point avant lui | **nous** | upstream jette la ligne entière |
| Crédit dans une réplique citée | **nous** | la réparation d'upstream réinsère le crédit |
| Crédit fait de prose pure, seul dans son paragraphe | **upstream**, une fois | `Edited by: faker` reste chez nous ; c'est le seul cas du corpus |
| Titre de chapitre seul dans son paragraphe | **nous** | upstream supprime les 22 titres du corpus, nous les gardons |
| Note de traducteur | **upstream**, une fois | une note sur 15 sort tronquée chez nous, et cette troncature est acceptée |

### Mesure : retirer la parenthèse qui suit un crédit

Idée mesurée : après un crédit, la parenthèse accolée ne dit rien de plus, et c'est
elle qui laisse `(Exclusive on)` en sortie.

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
| notre | `"Of course, there are some things that need to be fixed or tweaked. (Exclusive on) For example, the name 'slave' is a bit out of date."` |
| upstream | **identique au brut** |

Upstream coupe bien après `tweaked.`, puis son étape 3 voit le guillemet ouvert et
réinsère tout jusqu'au guillemet fermant : le crédit revient. Nous retirons le crédit
et laissons `(Exclusive on)`, qui est de la prose.

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
| Notes d'auteur et de traducteur (`Edited by: faker`, `T/N`, `E/N:`) | laissées | accepté : ce ne sont pas des références de site, elles peuvent partir sans dommage |
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
